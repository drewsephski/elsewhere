use agent_skills::{materialize_agents_skills, NativeSkillsLayout, SkillPackage};
use sqlx::PgPool;

async fn setup_pool() -> PgPool {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let pool = PgPool::connect(&url).await.expect("connect");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrate");
    pool
}

#[tokio::test]
async fn skill_version_is_immutable_on_run_snapshot() {
    let pool = setup_pool().await;
    let owner = format!("owner-{}", uuid::Uuid::new_v4());
    let bot_id = uuid::Uuid::new_v4().to_string();
    let computer_id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO sandboxes (id, owner_id, provider, provider_resource_id, state) VALUES ($1,$2,'mock','r','ready')")
        .bind(&computer_id).bind(&owner).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO bots (id, owner_id, name, system_prompt, model, computer_id) VALUES ($1,$2,'B','', 'gpt-5.6-luna', $3)")
        .bind(&bot_id).bind(&owner).bind(&computer_id).execute(&pool).await.unwrap();

    let md_v1 = "---\nname: competitor-research\ndescription: v1\n---\n\nv1\n";
    let package_v1 = SkillPackage::validate_and_build(md_v1, &[], Some("competitor-research")).unwrap();
    let (skill, _) = cloud_host::skills::create_skill_with_version(
        &pool,
        &owner,
        "competitor-research",
        &package_v1,
        &[],
    )
    .await
    .unwrap();
    cloud_host::skills::attach_bot_skill(&pool, &owner, &bot_id, &skill.id, None)
        .await
        .unwrap();

    let admission = cloud_host::skills::SkillAdmissionInput::default();
    let mut tx = pool.begin().await.unwrap();
    let records = cloud_host::work::enqueue_in_transaction(
        &mut tx,
        &owner,
        &uuid::Uuid::new_v4().to_string(),
        &bot_id,
        None,
        "Research competitors",
        &admission,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let packages = cloud_host::skills::load_run_skill_packages(&pool, &owner, &records.run_id)
        .await
        .unwrap();
    assert_eq!(packages.len(), 1);
    assert!(packages[0].skill_md.contains("v1"));

    let md_v2 = "---\nname: competitor-research\ndescription: v2\n---\n\nv2\n";
    let package_v2 = SkillPackage::validate_and_build(md_v2, &[], Some("competitor-research")).unwrap();
    cloud_host::skills::append_skill_version(&pool, &owner, &skill.id, &package_v2, &[])
        .await
        .unwrap();

    let packages_after = cloud_host::skills::load_run_skill_packages(&pool, &owner, &records.run_id)
        .await
        .unwrap();
    assert!(packages_after[0].skill_md.contains("v1"));
}

#[test]
fn materialize_agents_skills_layout() {
    let cwd = tempfile::tempdir().unwrap();
    let md = "---\nname: probe-skill\ndescription: d\n---\n\nbody\n";
    let package = SkillPackage::validate_and_build(md, &[], None).unwrap();
    materialize_agents_skills(cwd.path(), &[package]).unwrap();
    assert!(NativeSkillsLayout::agents_skills_root(cwd.path())
        .join("probe-skill/SKILL.md")
        .is_file());
}
