use crate::settings::triggers::a_train::ATrain;
use crate::settings::triggers::Trigger;
use crate::settings::Settings;

fn atrain() -> Trigger {
    Trigger::Atrain(ATrain::default())
}

fn manual() -> Trigger {
    serde_json::from_value::<Trigger>(serde_json::json!({ "type": "manual" }))
        .expect("manual trigger JSON should deserialize")
}

#[test]
fn noop_when_no_atrain_triggers_present() {
    let mut settings = Settings::default();
    let before = settings.triggers.clone();
    settings
        .ensure_atrain_alias()
        .expect("noop case should succeed");
    assert_eq!(before.len(), settings.triggers.len());
    assert!(!settings.triggers.contains_key("a-train"));
}

#[test]
fn noop_when_atrain_already_keyed_correctly() {
    let mut settings = Settings::default();
    settings.triggers.insert("a-train".to_string(), atrain());
    let len_before = settings.triggers.len();

    settings
        .ensure_atrain_alias()
        .expect("noop case should succeed");

    assert_eq!(
        settings.triggers.len(),
        len_before,
        "must not duplicate when key is already correct"
    );
    assert!(matches!(
        settings.triggers.get("a-train"),
        Some(Trigger::Atrain(_))
    ));
}

#[test]
fn aliases_misnamed_atrain_under_a_train_key() {
    let mut settings = Settings::default();
    settings.triggers.insert("my_drive".to_string(), atrain());

    settings
        .ensure_atrain_alias()
        .expect("single-misnamed case should succeed");

    assert!(
        matches!(settings.triggers.get("a-train"), Some(Trigger::Atrain(_))),
        "must install an `a-train` alias"
    );
    assert!(
        matches!(settings.triggers.get("my_drive"), Some(Trigger::Atrain(_))),
        "original key must be preserved so users can still reference it"
    );
}

#[test]
fn does_not_overwrite_existing_a_train_key() {
    // If a user has both `triggers.a-train` and `triggers.my_drive` of type
    // atrain, we keep the explicit `a-train` and just warn about the other.
    // The `excludes` sentinel lets us distinguish the kept-vs-overwritten
    // config without identity comparison.
    let mut settings = Settings::default();
    settings.triggers.insert(
        "a-train".to_string(),
        Trigger::Atrain(ATrain {
            excludes: vec!["sentinel".to_string()],
            ..ATrain::default()
        }),
    );
    settings.triggers.insert("my_drive".to_string(), atrain());

    settings
        .ensure_atrain_alias()
        .expect("collision case should warn-not-error");

    match settings.triggers.get("a-train") {
        Some(Trigger::Atrain(actual)) => assert_eq!(
            actual.excludes,
            vec!["sentinel".to_string()],
            "explicit a-train config must win over alias-source"
        ),
        other => panic!(
            "expected Trigger::Atrain at `a-train`, got present={}",
            other.is_some()
        ),
    }
}

#[test]
fn rejects_multiple_misnamed_atrain_triggers() {
    let mut settings = Settings::default();
    settings.triggers.insert("my_drive".to_string(), atrain());
    settings
        .triggers
        .insert("other_drive".to_string(), atrain());

    let err = settings
        .ensure_atrain_alias()
        .expect_err("ambiguous case must error");

    let msg = err.to_string();
    assert!(
        msg.contains("my_drive") && msg.contains("other_drive"),
        "error must name both conflicting triggers; got: {msg}"
    );
}

#[test]
fn ignores_non_atrain_a_train_keyed_trigger_but_completes() {
    // Edge case: user has `triggers.a-train: { type: manual }` plus a
    // misnamed atrain. We can't alias on top of the manual without
    // clobbering, so we just warn and return Ok.
    let mut settings = Settings::default();
    settings.triggers.insert("a-train".to_string(), manual());
    settings.triggers.insert("my_drive".to_string(), atrain());

    settings
        .ensure_atrain_alias()
        .expect("non-atrain `a-train` key should not be an error");

    assert!(matches!(
        settings.triggers.get("a-train"),
        Some(Trigger::Manual(_))
    ));
}
