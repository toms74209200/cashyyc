pub enum CharacterType {
    Lowercase,
    Uppercase,
    Numeric,
}

pub fn generate_random_string<R: std::io::Read>(
    length: usize,
    character_types: &[CharacterType],
    additional_characters: &str,
    source: &mut R,
) -> String {
    let pool: Vec<char> = character_types
        .iter()
        .flat_map(|t| match t {
            CharacterType::Lowercase => (b'a'..=b'z').map(|b| b as char).collect::<Vec<_>>(),
            CharacterType::Uppercase => (b'A'..=b'Z').map(|b| b as char).collect::<Vec<_>>(),
            CharacterType::Numeric => (b'0'..=b'9').map(|b| b as char).collect::<Vec<_>>(),
        })
        .chain(additional_characters.chars())
        .collect();

    let mut bytes = vec![0u8; length];
    source.read_exact(&mut bytes).unwrap();

    bytes
        .iter()
        .map(|b| pool[*b as usize % pool.len()])
        .collect()
}
