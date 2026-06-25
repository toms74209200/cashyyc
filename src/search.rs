const SCORE_MATCH: i32 = 16;
const BONUS_CONSECUTIVE: i32 = 4;
const BONUS_BOUNDARY: i32 = 8;
const BONUS_CAMEL: i32 = 4;
const FIRST_CHAR_MULTIPLIER: i32 = 2;
const NEG_INF: i32 = i32::MIN / 2;

fn boundary_bonus(raw: &[u8], j: usize) -> i32 {
    if j == 0 {
        return BONUS_BOUNDARY * FIRST_CHAR_MULTIPLIER;
    }
    let prev = raw[j - 1];
    let curr = raw[j];
    if matches!(prev, b'-' | b'_' | b'/' | b'.') {
        BONUS_BOUNDARY
    } else if curr.is_ascii_uppercase() && !prev.is_ascii_uppercase() {
        BONUS_CAMEL
    } else {
        0
    }
}

pub fn score(target: &str, query: &str) -> Option<i32> {
    let raw: Vec<u8> = target.bytes().collect();
    let t: Vec<u8> = raw.iter().map(|b| b.to_ascii_lowercase()).collect();
    let q: Vec<u8> = query.bytes().map(|b| b.to_ascii_lowercase()).collect();
    let (m, n) = (q.len(), t.len());

    if m == 0 {
        return Some(0);
    }
    if m > n {
        return None;
    }

    let mut dp = vec![vec![NEG_INF; n]; m];

    for j in 0..n {
        if t[j] == q[0] {
            dp[0][j] = SCORE_MATCH + boundary_bonus(&raw, j);
        }
    }

    for i in 1..m {
        let mut max_prev = NEG_INF;
        for j in 0..n {
            if j > 0 && dp[i - 1][j - 1] > max_prev {
                max_prev = dp[i - 1][j - 1];
            }

            if t[j] != q[i] {
                continue;
            }

            let pos_bonus = boundary_bonus(&raw, j);

            let consec = if j > 0 && dp[i - 1][j - 1] > NEG_INF {
                dp[i - 1][j - 1] + SCORE_MATCH + BONUS_CONSECUTIVE + pos_bonus
            } else {
                NEG_INF
            };

            let gap = if max_prev > NEG_INF {
                max_prev + SCORE_MATCH + pos_bonus
            } else {
                NEG_INF
            };

            dp[i][j] = consec.max(gap);
        }
    }

    let best = dp[m - 1].iter().copied().max()?;
    if best > NEG_INF { Some(best) } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_score_with_empty_query_then_some_zero() {
        assert_eq!(score("anything", ""), Some(0));
    }

    #[test]
    fn when_score_with_exact_match_then_some() {
        assert!(score("go", "go").is_some());
    }

    #[test]
    fn when_score_with_no_common_subsequence_then_none() {
        assert_eq!(score("rust", "xyz"), None);
    }

    #[test]
    fn when_score_with_query_longer_than_target_then_none() {
        assert_eq!(score("go", "gooo"), None);
    }

    #[test]
    fn when_score_matches_case_insensitively() {
        assert!(score("Go", "go").is_some());
        assert!(score("go", "Go").is_some());
    }

    #[test]
    fn when_score_prefers_consecutive_over_scattered() {
        let consec = score("acb", "ac").unwrap();
        let scattered = score("axc", "ac").unwrap();
        assert!(consec > scattered, "consec={consec} scattered={scattered}");
    }

    #[test]
    fn when_score_prefers_boundary_over_interior() {
        let boundary = score("go-lang", "go").unwrap();
        let interior = score("ergo", "go").unwrap();
        assert!(
            boundary > interior,
            "boundary={boundary} interior={interior}"
        );
    }

    #[test]
    fn when_score_prefers_separator_boundary() {
        let sep = score("x-go", "g").unwrap();
        let interior = score("xgo", "g").unwrap();
        assert!(sep > interior, "sep={sep} interior={interior}");
    }

    #[test]
    fn when_score_prefers_camel_boundary() {
        let camel = score("myCmd", "c").unwrap();
        let interior = score("mycmd", "c").unwrap();
        assert!(camel > interior, "camel={camel} interior={interior}");
    }

    #[test]
    fn when_score_first_char_gets_multiplied_bonus() {
        let first = score("go", "g").unwrap();
        let other = score("xg", "g").unwrap();
        assert!(first > other, "first={first} other={other}");
    }

    #[test]
    fn when_score_same_match_position_then_scores_equal() {
        let a = score("go", "go").unwrap();
        let b = score("go-postgres", "go").unwrap();
        assert_eq!(a, b, "a={a} b={b}");
    }

    #[test]
    fn when_score_ranks_devcontainer_templates_correctly() {
        let go = score("go", "go").unwrap();
        let go_pg = score("go-postgres", "go").unwrap();
        let django = score("django", "go").unwrap();
        assert!(go >= django, "go={go} django={django}");
        assert!(go_pg >= django, "go_pg={go_pg} django={django}");
    }
}
