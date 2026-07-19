fn fnv1a(s: &str) -> u64 {
    const OFFSET: u64 = 14695981039346656037;
    const PRIME: u64 = 1099511628211;
    s.bytes()
        .fold(OFFSET, |h, b| (h ^ b as u64).wrapping_mul(PRIME))
}

pub fn image_tag(local_folder: &std::path::Path) -> String {
    let raw = local_folder
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    let filtered: String = raw
        .chars()
        .filter(|c| matches!(c, 'a'..='z' | '0'..='9' | '.' | '_' | '-'))
        .collect();
    let name = filtered.trim_start_matches(['.', '-']);
    let name = if name.is_empty() { "workspace" } else { name };
    let hash = fnv1a(&local_folder.display().to_string().to_lowercase());
    format!("vsc-{}-{:016x}", name, hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_image_tag_then_starts_with_vsc_and_folder_name() {
        let tag = image_tag(std::path::Path::new("/home/user/myproject"));
        assert!(tag.starts_with("vsc-myproject-"));
    }

    #[test]
    fn when_image_tag_with_same_path_then_same_result() {
        let path = std::path::Path::new("/home/user/myproject");
        assert_eq!(image_tag(path), image_tag(path));
    }

    #[test]
    fn when_image_tag_with_different_paths_then_different_results() {
        let a = image_tag(std::path::Path::new("/home/user/project-a"));
        let b = image_tag(std::path::Path::new("/home/user/project-b"));
        assert_ne!(a, b);
    }

    #[test]
    fn when_image_tag_with_uppercase_folder_name_then_lowercase_in_tag() {
        let tag = image_tag(std::path::Path::new("/home/user/MyProject"));
        assert!(tag.starts_with("vsc-myproject-"));
    }

    #[test]
    fn when_image_tag_with_special_chars_in_folder_name_then_removed() {
        let tag = image_tag(std::path::Path::new("/home/user/my@project"));
        assert!(tag.starts_with("vsc-myproject-"));
    }

    #[test]
    fn when_image_tag_with_spaces_in_folder_name_then_removed() {
        let tag = image_tag(std::path::Path::new("/home/user/my project"));
        assert!(tag.starts_with("vsc-myproject-"));
    }

    #[test]
    fn when_image_tag_with_leading_hyphen_in_folder_name_then_trimmed() {
        let tag = image_tag(std::path::Path::new("/home/user/-myproject"));
        assert!(tag.starts_with("vsc-myproject-"));
    }

    #[test]
    fn when_image_tag_with_leading_period_in_folder_name_then_trimmed() {
        let tag = image_tag(std::path::Path::new("/home/user/.myproject"));
        assert!(tag.starts_with("vsc-myproject-"));
    }

    #[test]
    fn when_image_tag_with_only_invalid_chars_in_folder_name_then_workspace() {
        let tag = image_tag(std::path::Path::new("/home/user/@@@"));
        assert!(tag.starts_with("vsc-workspace-"));
    }
}
