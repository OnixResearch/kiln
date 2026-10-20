use hegel::TestCase;
use hegel::generators::sampled_from;
use ucan::core::CapabilityError;
use ucan::core::CapabilityField;
use ucan::core::CapabilityView;

const VALID_RESOURCES: [&str; 4] = [
    "did:key:z6Mkalpha",
    "did:key:z6Mkbravo",
    "did:key:z6Mkcharlie",
    "did:key:z6Mkdelta",
];
const VALID_NAMESPACES: [&str; 4] = ["store", "upload", "queue", "email"];
const VALID_ACTIONS: [&str; 4] = ["add", "read", "write", "*"];
const EMBEDDED_WILDCARD_NAMESPACES: [&str; 3] = ["sto*re", "upl*oad", "que*ue"];
const INVALID_ABILITIES: [&str; 4] = ["store//add", "store/Add", " store/add", "store/add "];
const INVALID_WILDCARD_CHARACTER: char = '*';
const INVALID_UPPERCASE_CHARACTER: char = 'A';

#[hegel::test]
fn prop_valid_capability_parts_round_trip(tc: TestCase) {
    let resource = tc.draw(sampled_from(VALID_RESOURCES.to_vec()));
    let namespace = tc.draw(sampled_from(VALID_NAMESPACES.to_vec()));
    let action = tc.draw(sampled_from(VALID_ACTIONS.to_vec()));
    let ability = format!("{namespace}/{action}");

    let capability =
        CapabilityView::new(resource, ability.as_str()).expect("generated capability parts should validate");

    assert_eq!(capability.resource(), resource);
    assert_eq!(capability.ability(), ability.as_str());
}

#[hegel::test]
fn prop_embedded_wildcards_are_rejected(tc: TestCase) {
    let resource = tc.draw(sampled_from(VALID_RESOURCES.to_vec()));
    let namespace = tc.draw(sampled_from(EMBEDDED_WILDCARD_NAMESPACES.to_vec()));
    let ability = format!("{namespace}/add");

    let error =
        CapabilityView::new(resource, ability.as_str()).expect_err("embedded wildcard segments must fail validation");

    assert_eq!(error, CapabilityError::InvalidAbilityCharacter {
        character: INVALID_WILDCARD_CHARACTER,
    });
}

#[hegel::test]
fn prop_known_invalid_ability_forms_are_rejected(tc: TestCase) {
    let resource = tc.draw(sampled_from(VALID_RESOURCES.to_vec()));
    let ability = tc.draw(sampled_from(INVALID_ABILITIES.to_vec()));

    let error = CapabilityView::new(resource, ability).expect_err("known-invalid ability forms must fail validation");

    match ability {
        "store//add" => assert_eq!(error, CapabilityError::EmptyAbilitySegment),
        "store/Add" => assert_eq!(error, CapabilityError::InvalidAbilityCharacter {
            character: INVALID_UPPERCASE_CHARACTER,
        }),
        " store/add" | "store/add " => assert_eq!(error, CapabilityError::FieldMustBeTrimmed {
            field: CapabilityField::Ability,
        }),
        _ => panic!("unexpected invalid fixture: {ability}"),
    }
}
