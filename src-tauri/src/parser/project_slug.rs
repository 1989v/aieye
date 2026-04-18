use std::path::PathBuf;

pub fn decode_project_slug(slug: &str) -> Option<PathBuf> {
    decode_with_home(slug, std::env::var("HOME").ok().as_deref())
}

pub fn decode_with_home(slug: &str, home: Option<&str>) -> Option<PathBuf> {
    if !slug.starts_with('-') {
        return None;
    }

    // HOME-aware heuristic: slug 의 home 부분은 원래 경로 유지.
    // 예: /Users/gideok-kwon 이 home 인데 slug 에서 "Users-gideok-kwon" 구간은
    // 원래 경로에 - 가 있을 수도 있으므로 그대로 복원.
    if let Some(home) = home {
        let home_slug = home.replace('/', "-"); // "/Users/gideok-kwon" → "-Users-gideok-kwon"
        if slug.starts_with(&home_slug) {
            let rest = &slug[home_slug.len()..];
            let rest_path = rest.replace('-', "/");
            return Some(PathBuf::from(format!("{home}{rest_path}")));
        }
    }

    // fallback: 모든 `-` → `/`
    let naive = slug.replace('-', "/");
    Some(PathBuf::from(naive))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_with_home_preserves_home_dashes() {
        let path = decode_with_home(
            "-Users-gideok-kwon-IdeaProjects-msa",
            Some("/Users/gideok-kwon"),
        );
        assert_eq!(
            path,
            Some(PathBuf::from("/Users/gideok-kwon/IdeaProjects/msa"))
        );
    }

    #[test]
    fn decodes_without_home_falls_back_to_naive() {
        let path = decode_with_home(
            "-Users-gideok-kwon-IdeaProjects-msa",
            None,
        );
        assert_eq!(
            path,
            Some(PathBuf::from("/Users/gideok/kwon/IdeaProjects/msa"))
        );
    }

    #[test]
    fn returns_none_for_non_slug() {
        assert_eq!(decode_with_home("no-leading-dash", None), None);
    }
}
