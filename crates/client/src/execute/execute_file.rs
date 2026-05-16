use super::shared::{clean_value, payload_field, run_process, split_args};

pub(super) fn handle(payload: &str) -> String {
    let path = payload_field(payload, "path")
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            let trimmed = payload.trim();
            (!trimmed.is_empty() && !trimmed.lines().all(|line| line.contains('=')))
                .then(|| trimmed.to_string())
        });
    let Some(path) = path else {
        return "execute_file\nstatus=failed\nmessage=path is required".to_string();
    };
    let args = payload_field(payload, "args")
        .map(|value| split_args(&value))
        .unwrap_or_default();
    let working_dir =
        payload_field(payload, "working_dir").filter(|value| !value.trim().is_empty());
    let output = run_process(&path, &args, working_dir.as_deref());
    format!(
        "execute_file\npath={}\nargs={}\n{}",
        clean_value(&path),
        clean_value(&args.join(" ")),
        output
    )
}
