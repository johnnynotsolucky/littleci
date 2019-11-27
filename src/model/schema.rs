table! {
    queue (id) {
        id -> Text,
        status -> Text,
        exit_code -> Nullable<Integer>,
        data -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        repository_id -> Text,
    }
}

table! {
    queue_logs (id) {
        id -> Integer,
        status -> Text,
        exit_code -> Nullable<Integer>,
        created_at -> Timestamp,
        queue_id -> Text,
    }
}

table! {
    repositories (id) {
        id -> Text,
        slug -> Text,
        name -> Text,
        run -> Text,
        working_dir -> Nullable<Text>,
        secret -> Text,
        variables -> Nullable<Text>,
        triggers -> Nullable<Text>,
        webhooks -> Nullable<Text>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    users (id) {
        id -> Text,
        username -> Text,
        password -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

joinable!(queue -> repositories (repository_id));
joinable!(queue_logs -> queue (queue_id));

allow_tables_to_appear_in_same_query!(
    queue,
    queue_logs,
    repositories,
    users,
);
