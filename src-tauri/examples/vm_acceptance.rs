//! `cargo run --example vm_acceptance -- <app_data_dir> <provision|write-proof|read-proof|stop>`

use gptbot_lib::vm::{GuestRequest, VirtualMachineManager};
use std::path::PathBuf;
use std::process;

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        return Err("usage: vm_acceptance <app_data_dir> <command>".into());
    }
    let app_data = PathBuf::from(&args[1]);
    let command = &args[2];
    let vm = VirtualMachineManager::new(&app_data);

    match command.as_str() {
        "provision" => {
            vm.provision()?;
            Ok(())
        }
        "stop" => {
            vm.stop()?;
            Ok(())
        }
        "write-proof" => {
            vm.start()?;
            let response = vm.guest_request(GuestRequest {
                id: "write-proof".into(),
                method: "exec".into(),
                params: serde_json::json!({
                    "command": "mkdir -p /workspace && echo \"hello from the persistent GPT Bot computer\" > /workspace/proof.txt"
                }),
            })?;
            if !response.ok {
                return Err(response
                    .error
                    .unwrap_or_else(|| "write failed".into()));
            }
            vm.stop()?;
            Ok(())
        }
        "read-proof" => {
            vm.start()?;
            let response = vm.guest_request(GuestRequest {
                id: "read-proof".into(),
                method: "exec".into(),
                params: serde_json::json!({
                    "command": "cat /workspace/proof.txt"
                }),
            })?;
            let stdout = response.stdout.unwrap_or_default();
            if stdout.trim() != "hello from the persistent GPT Bot computer" {
                return Err(format!("unexpected proof contents: {stdout}"));
            }
            println!("proof ok: {}", stdout.trim());
            vm.stop()?;
            Ok(())
        }
        other => Err(format!("unknown command: {other}")),
    }
}
