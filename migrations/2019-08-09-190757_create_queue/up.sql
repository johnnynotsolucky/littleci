CREATE TABLE users (
	id VARCHAR PRIMARY KEY NOT NULL,
	username VARCHAR NOT NULL,
	password VARCHAR NOT NULL,
	created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
	updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE repositories (
	id VARCHAR PRIMARY KEY NOT NULL,
	slug VARCHAR NOT NULL,
	name VARCHAR NOT NULL,
	run VARCHAR NOT NULL,
	working_dir VARCHAR,
	secret VARCHAR NOT NULL,
	variables TEXT,
	triggers TEXT,
	webhooks TEXT,
	deleted INTEGER NOT NULL DEFAULT 0,
	created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
	updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE queue (
	id VARCHAR PRIMARY KEY NOT NULL,
	status VARCHAR NOT NULL,
	exit_code INTEGER,
	data TEXT NOT NULL,
	created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
	updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
	repository_id VARCHAR NOT NULL,
	CONSTRAINT fk_repository
		FOREIGN KEY(repository_id)
		REFERENCES repositories(id)
		ON DELETE CASCADE
);

CREATE TABLE queue_logs (
	id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
	status VARCHAR NOT NULL,
	exit_code INTEGER,
	created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
	queue_id VARCHAR NOT NULL,
	CONSTRAINT fk_queue
		FOREIGN KEY(queue_id)
		REFERENCES queue(id)
		ON DELETE CASCADE
);
