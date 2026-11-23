const EXCESS_LABEL_BYTES: usize = crate::MAXIMUM_LABEL_BYTES.strict_add(1);
const EXCESS_TEXT_BYTES: usize = crate::MAXIMUM_TEXT_BYTES.strict_add(1);

#[test]
fn accepts_canonical_values_without_changing_content() -> Result<(), crate::ValueError> {
    let identity_text = alloc::format!("b3:{}", "a".repeat(crate::BLAKE3_HEX_LENGTH));
    let identity = crate::Identity::new(identity_text.clone())?;
    assert_eq!(identity.as_str(), identity_text);

    let label_text = "a".repeat(crate::MAXIMUM_LABEL_BYTES);
    let label = crate::Label::new(label_text.clone())?;
    assert_eq!(label.as_str(), label_text);

    let bounded_text = "a".repeat(crate::MAXIMUM_TEXT_BYTES);
    let text = crate::BoundedText::new(bounded_text.clone())?;
    assert_eq!(text.as_str(), bounded_text);

    let revision_text = "a".repeat(crate::GIT_REVISION_HEX_LENGTH);
    let revision = crate::GitRevision::new(revision_text.clone())?;
    assert_eq!(revision.as_str(), revision_text);
    Ok(())
}

#[test]
fn rejects_malformed_input() {
    // r[verify cairn.gate_interchange.conformance.scenario.negative]
    assert_eq!(
        crate::Identity::new("not-an-identity".into()),
        Err(crate::ValueError::InvalidIdentity)
    );
    assert_eq!(
        crate::Label::new("Uppercase".into()),
        Err(crate::ValueError::InvalidLabel)
    );
    assert_eq!(
        crate::BoundedText::new("line\nbreak".into()),
        Err(crate::ValueError::InvalidText)
    );
    assert_eq!(
        crate::GitRevision::new("A".repeat(crate::GIT_REVISION_HEX_LENGTH)),
        Err(crate::ValueError::InvalidGitRevision)
    );
}

#[test]
fn rejects_empty_and_overlong_values() {
    assert_eq!(
        crate::Label::new(alloc::string::String::new()),
        Err(crate::ValueError::InvalidLabel)
    );
    assert_eq!(
        crate::Label::new("a".repeat(EXCESS_LABEL_BYTES)),
        Err(crate::ValueError::InvalidLabel)
    );
    assert_eq!(
        crate::BoundedText::new(alloc::string::String::new()),
        Err(crate::ValueError::InvalidText)
    );
    assert_eq!(
        crate::BoundedText::new("a".repeat(EXCESS_TEXT_BYTES)),
        Err(crate::ValueError::InvalidText)
    );
    assert_eq!(
        crate::BoundedText::new("é".repeat(crate::MAXIMUM_TEXT_BYTES)),
        Err(crate::ValueError::InvalidText)
    );
}
