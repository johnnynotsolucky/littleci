use reqwest::Client;
use serde::Serialize;
use serde_json::to_string as to_json_string;
use std::convert::From;
use std::fmt::Debug;
use std::fs::{create_dir_all, File};
use std::process::{Command, Stdio};
use std::thread;

#[allow(unused_imports)]
use log::{debug, error, info, warn};

use super::{ExecutionStatus, QueueItem, QueueService};
use crate::model::queues::Queues;
use crate::model::repositories::{Repositories, Repository};

#[derive(Serialize, Debug, Clone)]
pub struct QueueItemData {
	/// A random system-generated execution identifier.
	pub id: String,

	/// Repository identifier
	pub repository: String,

	/// Current status of the execution
	#[serde(flatten)]
	pub status: ExecutionStatus,
}

impl From<QueueItem> for QueueItemData {
	fn from(queue_item: QueueItem) -> Self {
		Self {
			id: queue_item.id,
			repository: queue_item.repository_id,
			status: queue_item.status.clone(),
		}
	}
}

const SUCCESS_EXIT_CODE: i32 = 0;

pub trait JobRunner: Debug + Send + Sync {
	fn process(&self, queue_service: QueueService);
}

#[derive(Debug, Clone)]
pub struct CommandRunner;

impl JobRunner for CommandRunner {
	fn process(&self, queue_service: QueueService) {
		thread::spawn(move || {
			let queue_name = queue_service.name.clone();
			let queue_name = (&*queue_name).to_owned();
			let processing_queue = queue_service.processing_queue.clone();

			// Acquire a lock so that we can ensure that only a single thread per queue is spawned.
			// When `notify` is called we can try acquire a lock, if unsuccessful we can assume that
			// there is already a thread processing the queue.
			let lock = processing_queue.try_lock();

			if lock.is_some() {
				debug!("Queue {} checking for new jobs", queue_name);

				loop {
					// Refresh the repository in case it changed between builds
					let repository = Repositories::new(queue_service.config.clone())
						.find_by_id(&queue_service.repository_id);

					let repository = match repository {
						Some(repository) => repository,
						None => {
							error!(
								"Could not find repository with ID {}",
								&queue_service.repository_id
							);
							return;
						}
					};

					let queue_model = Queues::new(queue_service.config.clone());
					let item = queue_model.next_queued(&repository.id);

					match item {
						Some(mut item) => {
							info!("Starting execution {}", &item.id);
							item.status = ExecutionStatus::Running;

							if let Err(error) = queue_model.update_status(&item) {
								error!("Unable to update status of job {}. {}", &item.id, error);
							}

							call_webhooks(&repository, &item);

							let execution_dir =
								format!("{}/jobs/{}", &queue_service.config.data_dir, &item.id);

							match create_dir_all(&execution_dir) {
								Ok(_) => {
									let stdout_log_f = File::create(format!("{}/output.log", &execution_dir));

									let stdout_log_f = match stdout_log_f {
										Ok(stdio) => stdio,
										_ => {
											error!("Unable to create stdout log file");
											return
										},
									};

									let stderr_log_f = stdout_log_f.try_clone();

									let stderr_log_f = match stderr_log_f {
										Ok(stdio) => stdio,
										_ => {
											error!("Unable to create stderr log file");
											return
										},
									};

									let mut command = Command::new("/bin/sh");

									for variable in repository.variables.iter() {
										let (key, value) = variable;
										command.env(key, value);
									}

									let data = &item.data.inner();
									for (key, value) in data.iter() {
										command.env(key, value);
									}

									if let Some(working_dir) = &repository.working_dir {
										command.current_dir(working_dir.to_owned());
									};

									command
										.args(&["-c", &repository.run.to_string()])
										.stdout(Stdio::from(stdout_log_f))
										.stderr(Stdio::from(stderr_log_f));

									let status = command.status();

									match status {
										Ok(status) => {
											match status.code() {
												Some(code) => {
													match code {
														code if code != SUCCESS_EXIT_CODE => {
															item.status = ExecutionStatus::Failed(code);
															if let Err(error) = queue_model.update_status(&item) {
																error!("Unable to update status of job {}. {}", &item.id, error);
															}
															error!("Exection {} failed with code {}", &item.id, code)
														},
														_ => {
															item.status = ExecutionStatus::Completed;
															if let Err(error) = queue_model.update_status(&item) {
																error!("Unable to update status of item {}. {}", &item.id, error);
															}
															info!("Execution {} completed successfully", &item.id)
														},
													}
												},
												None => {
													item.status = ExecutionStatus::Cancelled;
													if let Err(error) = queue_model.update_status(&item) {
														error!("Unable to update status of item {}. {}", &item.id, error);
													}
													info!("Exection {} terminated by signal", &item.id)
												},
											}
										},
										Err(error) => {
											item.status = ExecutionStatus::Failed(-1);
											if let Err(error) = queue_model.update_status(&item) {
												error!("Unable to update status of item {}. {}", &item.id, error);
											}
											error!("Execution {} failed. Unable to launch script. Error: {}", &item.id, error)
										},
									}
								},
								Err(_) => error!("Execution {} failed. Unable to create log dir. Please check permissions.", &item.id),
							}

							call_webhooks(&repository, &item);
						}
						// We've processed all the items in this queue and can exit
						None => break,
					}
				}

				debug!(
					"Finished processing queue {}. Terminating thread.",
					queue_name
				);
			} else {
				debug!("Queue {} is aleady processing jobs", queue_name);
			}
		});
	}
}

fn call_webhooks(repository: &Repository, item: &QueueItem) {
	let client = Client::new();
	match to_json_string(&QueueItemData::from(item.clone())) {
		Ok(json_data) => {
			for webhook in repository.webhooks.iter() {
				let res = client.post(webhook).body(json_data.clone()).send();

				match res {
					Ok(_) => info!("Webhook called: {}", webhook),
					Err(error) => error!("Webhook failed: {}. {}", webhook, error),
				}
			}
		}
		Err(error) => error!("Unable to serialize job data. {}", error),
	}
}
