#[macro_export]
macro_rules! define_task_result {
    ($t:ty) => {
        struct TaskResult {
            data: Option<$t>,
            work_id: Option<usize>,
            t: datatypes::EventTime,
        }
    };
}

#[macro_export]
macro_rules! terminate_processor {
    ($processor:path, $channel:path) => {
        use pl::datatypes::TimeFormat;
        $channel
            .send(TaskResult {
                data: None,
                work_id: None,
                t: EventTime::new(TimeFormat::Omit),
            })
            .unwrap();
        $processor.join().unwrap();
    };
}

#[macro_export]
macro_rules! define_de_source {
    ($default_port:path) => {
        use pl::tools::{HostPort, ParseHostPort};
        fn de_source<'de, D>(deserializer: D) -> serde::export::Result<HostPort, D::Error>
        where
            D: Deserializer<'de>,
        {
            return Ok(String::deserialize(deserializer)
                .unwrap()
                .parse_host_port($default_port));
        }
    };
}
