//! Baseline-relative git diff collection against a real shell workspace.

mod github_coding_shell;

use agent_core::AgentComputer;
use cloud_host::github_coding::workspace_git::{
    collect_publish_changes, init_baseline_repo, reset_checkout_dir,
};
use github_coding_shell::ShellWorkspaceComputer;

async fn baseline_checkout(computer: &ShellWorkspaceComputer, suffix: &str) -> (String, String) {
    let checkout = format!("/workspace/repos/acme/demo/{}", suffix);
    reset_checkout_dir(computer, &checkout).await.unwrap();
    computer
        .write_file(&format!("{}/README.md", checkout), b"Hello")
        .await
        .unwrap();
    let baseline = init_baseline_repo(computer, &checkout).await.unwrap();
    (checkout, baseline)
}

#[tokio::test]
async fn baseline_diff_modified_tracked_file() {
    let computer = ShellWorkspaceComputer::new();
    let (checkout, baseline) = baseline_checkout(&computer, "mod").await;
    computer
        .write_file(&format!("{}/README.md", checkout), b"Hello from Elsewhere")
        .await
        .unwrap();
    let changes = collect_publish_changes(&computer, &checkout, &baseline)
        .await
        .unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].path, "README.md");
    assert!(!changes[0].deleted);
    assert_eq!(changes[0].bytes.as_deref(), Some(b"Hello from Elsewhere".as_ref()));
}

#[tokio::test]
async fn baseline_diff_new_file_and_directory() {
    let computer = ShellWorkspaceComputer::new();
    let (checkout, baseline) = baseline_checkout(&computer, "newdir").await;
    computer
        .write_file(&format!("{}/nested/dir/file.txt", checkout), b"nested")
        .await
        .unwrap();
    let changes = collect_publish_changes(&computer, &checkout, &baseline)
        .await
        .unwrap();
    assert!(changes.iter().any(|c| c.path == "nested/dir/file.txt"));
}

#[tokio::test]
async fn baseline_diff_deleted_file() {
    let computer = ShellWorkspaceComputer::new();
    let (checkout, baseline) = baseline_checkout(&computer, "del").await;
    computer
        .exec(&format!("rm {}", format!("{}/README.md", checkout)))
        .await
        .unwrap();
    let changes = collect_publish_changes(&computer, &checkout, &baseline)
        .await
        .unwrap();
    assert_eq!(changes.len(), 1);
    assert!(changes[0].deleted);
    assert_eq!(changes[0].path, "README.md");
}

#[tokio::test]
async fn baseline_diff_executable_mode() {
    let computer = ShellWorkspaceComputer::new();
    let (checkout, baseline) = baseline_checkout(&computer, "chmod").await;
    computer
        .exec(&format!("chmod +x {}/README.md", checkout))
        .await
        .unwrap();
    let changes = collect_publish_changes(&computer, &checkout, &baseline)
        .await
        .unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].mode, "100755");
}

#[tokio::test]
async fn local_git_commit_does_not_hide_baseline_diff() {
    let computer = ShellWorkspaceComputer::new();
    let (checkout, baseline) = baseline_checkout(&computer, "commit").await;
    computer
        .write_file(&format!("{}/README.md", checkout), b"Changed")
        .await
        .unwrap();
    computer
        .exec(&format!(
            "cd {} && git add -A && git commit -q -m 'bot commit'",
            checkout
        ))
        .await
        .unwrap();
    let changes = collect_publish_changes(&computer, &checkout, &baseline)
        .await
        .unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].bytes.as_deref(), Some(b"Changed".as_ref()));
}
