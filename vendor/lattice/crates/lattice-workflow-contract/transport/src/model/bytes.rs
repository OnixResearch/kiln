use base64::Engine;

pub fn serialize<S: serde::Serializer>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    serializer.serialize_str(&encoded)
}

pub fn deserialize<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
    let encoded = <String as serde::Deserialize>::deserialize(deserializer)?;
    base64::engine::general_purpose::STANDARD.decode(encoded).map_err(serde::de::Error::custom)
}
