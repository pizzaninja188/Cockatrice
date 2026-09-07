use prost::Message;
use std::path::PathBuf;
use tricerules_server::replay::{build_resume_plan, load_capture, reconstruct, ReplayOptions};

fn run() -> Result<bool, String> {
    let mut args = std::env::args().skip(1);
    let mut capture = None;
    let mut output = None;
    let mut options = ReplayOptions::default();
    let mut resume_plan = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--capture" => {
                capture = Some(PathBuf::from(
                    args.next().ok_or("--capture requires a directory")?,
                ))
            }
            "--output" => {
                output = Some(PathBuf::from(
                    args.next().ok_or("--output requires a directory")?,
                ))
            }
            "--stop-after" => {
                options.stop_after = Some(
                    args.next()
                        .ok_or("--stop-after requires a count")?
                        .parse()
                        .map_err(|_| "invalid count")?,
                )
            }
            "--stop-before" => {
                let count: u64 = args
                    .next()
                    .ok_or("--stop-before requires a one-based accepted-command count")?
                    .parse()
                    .map_err(|_| "invalid count")?;
                options.stop_after = Some(count.checked_sub(1).ok_or("--stop-before starts at 1")?);
            }
            "--retry-sequence" => {
                options.retry_sequence = Some(
                    args.next()
                        .ok_or("--retry-sequence requires a sequence")?
                        .parse()
                        .map_err(|_| "invalid sequence")?,
                )
            }
            "--allow-build-mismatch" => options.allow_build_mismatch = true,
            "--resume-plan" => resume_plan = true,
            "--help" | "-h" => {
                println!("tricerules-replay --capture DIRECTORY [--stop-after ACCEPTED_COUNT | --stop-before ACCEPTED_COUNT | --retry-sequence REQUEST_SEQUENCE] [--output NEW_DIRECTORY] [--resume-plan] [--allow-build-mismatch]\nExit codes: 0 matching accepted prefix, 2 differences, 1 invalid/incompatible capture. Output files contain server-only hidden information.");
                return Ok(true);
            }
            _ => return Err(format!("unknown argument: {arg}")),
        }
    }
    if resume_plan && output.is_none() {
        return Err("--resume-plan requires a new --output directory".into());
    }
    let capture = load_capture(&capture.ok_or("--capture is required")?)?;
    let result = reconstruct(&capture, &options)?;
    let summary = serde_json::json!({"format_version":1,"capture_id":capture.manifest["capture_id"],
        "accepted_commands":result.accepted_commands.to_string(),"matches":result.differences.is_empty(),
        "first_difference":result.differences.first(),"warnings":result.warnings,"retry_response":result.retry});
    if let Some(output) = output {
        if output.exists() {
            return Err(
                "Output directory already exists; choose a new path to preserve previous evidence"
                    .into(),
            );
        }
        std::fs::create_dir_all(&output).map_err(|e| e.to_string())?;
        if resume_plan {
            let plan = build_resume_plan(&capture, &options)?;
            std::fs::write(output.join("resume-plan.pb"), plan.encode_to_vec())
                .map_err(|e| e.to_string())?;
            std::fs::write(
                output.join("resume-plan.json"),
                serde_json::to_vec_pretty(
                    &tricerules_core::diagnostic_json::to_value(&plan)
                        .map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
        }
        for (name, value) in [
            ("summary.json", &summary),
            ("engine-state.json", &result.state),
            (
                "differences.json",
                &serde_json::Value::Array(result.differences.clone()),
            ),
        ] {
            std::fs::write(
                output.join(name),
                serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())?
    );
    Ok(result.differences.is_empty())
}

fn main() {
    match run() {
        Ok(true) => (),
        Ok(false) => std::process::exit(2),
        Err(error) => {
            eprintln!("{}", serde_json::json!({"error":error}));
            std::process::exit(1);
        }
    }
}
