table! {
    queue (id) {
        id -> Text,
        repository -> Text,
        status -> Text,
        exit_code -> Nullable<Integer>,
        data -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
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

joinable!(queue_logs -> queue (queue_id));

allow_tables_to_appear_in_same_query!(
    queue,
    queue_logs,
);
