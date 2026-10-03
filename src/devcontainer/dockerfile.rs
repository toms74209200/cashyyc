use std::collections::HashMap;
use std::ops::ControlFlow;

pub struct Dockerfile {
    args: Vec<Arg>,
    stages: Vec<Stage>,
}

struct Arg {
    name: String,
    default: Option<Word>,
}

struct Stage {
    base: Word,
    name: Option<String>,
}

struct Word(Vec<Part>);

enum Part {
    Literal(String),
    Variable { name: String, expansion: Expansion },
}

enum Expansion {
    Value,
    Default(Word),
    Alternative(Word),
}

impl Dockerfile {
    pub fn parse(text: &str) -> Option<Self> {
        text.lines().try_fold(
            Dockerfile {
                args: vec![],
                stages: vec![],
            },
            |dockerfile, line| {
                let tokens: Vec<&str> = line.split_whitespace().collect();
                match (
                    tokens
                        .first()
                        .map(|keyword| keyword.to_ascii_uppercase())
                        .as_deref(),
                    dockerfile.stages.is_empty(),
                ) {
                    (Some("FROM"), _) => {
                        let stage = match &tokens[1..]
                            .iter()
                            .skip_while(|token| token.starts_with("--platform="))
                            .collect::<Vec<_>>()[..]
                        {
                            [image, keyword, name, ..] if keyword.eq_ignore_ascii_case("AS") => {
                                Stage {
                                    base: Word::parse(image)?,
                                    name: Some(name.to_string()),
                                }
                            }
                            [image, ..] => Stage {
                                base: Word::parse(image)?,
                                name: None,
                            },
                            [] => None?,
                        };
                        Some(Dockerfile {
                            stages: dockerfile.stages.into_iter().chain([stage]).collect(),
                            ..dockerfile
                        })
                    }
                    (Some("ARG"), true) => Some(Dockerfile {
                        args: dockerfile
                            .args
                            .into_iter()
                            .map(Some)
                            .chain(tokens[1..].iter().map(|token| {
                                Some(match token.split_once('=') {
                                    Some((name, default)) => Arg {
                                        name: name.to_string(),
                                        default: Some(Word::parse(default)?),
                                    },
                                    None => Arg {
                                        name: token.to_string(),
                                        default: None,
                                    },
                                })
                            }))
                            .collect::<Option<_>>()?,
                        ..dockerfile
                    }),
                    _ => Some(dockerfile),
                }
            },
        )
    }

    pub fn base_image(
        &self,
        build_args: &HashMap<String, String>,
        target: Option<&str>,
    ) -> Option<String> {
        let arch = match std::env::consts::ARCH {
            "x86_64" => "amd64",
            "aarch64" => "arm64",
            "x86" => "386",
            other => other,
        };
        let variables = self.args.iter().fold(
            HashMap::from([
                ("BUILDPLATFORM".to_string(), format!("linux/{arch}")),
                ("BUILDOS".to_string(), "linux".to_string()),
                ("BUILDARCH".to_string(), arch.to_string()),
                ("BUILDVARIANT".to_string(), String::new()),
                ("TARGETPLATFORM".to_string(), format!("linux/{arch}")),
                ("TARGETOS".to_string(), "linux".to_string()),
                ("TARGETARCH".to_string(), arch.to_string()),
                ("TARGETVARIANT".to_string(), String::new()),
            ]),
            |variables, arg| match (build_args.get(&arg.name), &arg.default) {
                (Some(value), _) => variables
                    .into_iter()
                    .chain([(arg.name.clone(), value.clone())])
                    .collect(),
                (None, Some(default)) => {
                    let value = default.evaluate(&variables);
                    variables
                        .into_iter()
                        .chain([(arg.name.clone(), value)])
                        .collect()
                }
                (None, None) => variables,
            },
        );
        match (0..self.stages.len()).try_fold(
            match target {
                Some(target) => self
                    .stages
                    .iter()
                    .rfind(|stage| stage.name.as_deref() == Some(target))?,
                None => self.stages.last()?,
            },
            |stage, _| {
                let image = stage.base.evaluate(&variables);
                match self
                    .stages
                    .iter()
                    .rfind(|stage| stage.name.as_deref() == Some(image.as_str()))
                {
                    Some(next) => ControlFlow::Continue(next),
                    None => ControlFlow::Break(image),
                }
            },
        ) {
            ControlFlow::Break(image) => Some(image),
            ControlFlow::Continue(_) => None,
        }
    }
}

impl Word {
    fn parse(text: &str) -> Option<Self> {
        match text.chars().next() {
            None => Some(Word(vec![])),
            Some('"' | '\'') => Word::parse(&text[1..]),
            Some('$') => {
                let rest = &text[1..];
                let (part, after) = match rest.strip_prefix('{') {
                    Some(braced) => {
                        let close = braced
                            .char_indices()
                            .scan(1usize, |depth, (i, c)| {
                                *depth = match c {
                                    '{' => *depth + 1,
                                    '}' => *depth - 1,
                                    _ => *depth,
                                };
                                Some((i, *depth))
                            })
                            .find(|(_, depth)| *depth == 0)?
                            .0;
                        let inner = &braced[..close];
                        let name_len = inner
                            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                            .unwrap_or(inner.len());
                        let expansion = match (&inner[..name_len], &inner[name_len..]) {
                            ("", _) => None,
                            (_, "") => Some(Expansion::Value),
                            (_, modifier) => match modifier.split_at(modifier.len().min(2)) {
                                (":-", word) => Some(Expansion::Default(Word::parse(word)?)),
                                (":+", word) => Some(Expansion::Alternative(Word::parse(word)?)),
                                _ => None,
                            },
                        }?;
                        (
                            Part::Variable {
                                name: inner[..name_len].to_string(),
                                expansion,
                            },
                            &braced[close + 1..],
                        )
                    }
                    None => {
                        let name_len = rest
                            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                            .unwrap_or(rest.len());
                        match name_len {
                            0 => (Part::Literal("$".to_string()), rest),
                            _ => (
                                Part::Variable {
                                    name: rest[..name_len].to_string(),
                                    expansion: Expansion::Value,
                                },
                                &rest[name_len..],
                            ),
                        }
                    }
                };
                Some(Word(
                    [part].into_iter().chain(Word::parse(after)?.0).collect(),
                ))
            }
            Some(_) => {
                let end = text.find(['$', '"', '\'']).unwrap_or(text.len());
                Some(Word(
                    [Part::Literal(text[..end].to_string())]
                        .into_iter()
                        .chain(Word::parse(&text[end..])?.0)
                        .collect(),
                ))
            }
        }
    }

    fn evaluate(&self, variables: &HashMap<String, String>) -> String {
        self.0
            .iter()
            .map(|part| match part {
                Part::Literal(text) => text.clone(),
                Part::Variable { name, expansion } => {
                    match (
                        variables.get(name).filter(|value| !value.is_empty()),
                        expansion,
                    ) {
                        (Some(value), Expansion::Value | Expansion::Default(_)) => value.clone(),
                        (None, Expansion::Value | Expansion::Alternative(_)) => String::new(),
                        (None, Expansion::Default(word))
                        | (Some(_), Expansion::Alternative(word)) => word.evaluate(variables),
                    }
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use random_string::{CharacterType, generate_random_string};
    use std::fs::File;

    #[test]
    fn when_base_image_with_from_image_then_returns_the_image() {
        let image = generate_random_string(
            8,
            &[CharacterType::Lowercase],
            "",
            &mut File::open("/dev/urandom").unwrap(),
        );
        let dockerfile = Dockerfile::parse(&format!("FROM {image}\n")).unwrap();

        let actual = dockerfile.base_image(&HashMap::new(), None);

        assert_eq!(actual, Some(image));
    }

    #[test]
    fn when_base_image_with_arg_default_in_from_then_returns_the_arg_default() {
        let image = generate_random_string(
            8,
            &[CharacterType::Lowercase],
            "",
            &mut File::open("/dev/urandom").unwrap(),
        );
        let dockerfile =
            Dockerfile::parse(&format!("ARG BASE_IMAGE={image}\nFROM ${{BASE_IMAGE}}\n")).unwrap();

        let actual = dockerfile.base_image(&HashMap::new(), None);

        assert_eq!(actual, Some(image));
    }

    #[test]
    fn when_base_image_with_quoted_arg_default_in_from_then_returns_the_arg_default_without_quotes()
    {
        let image = generate_random_string(
            8,
            &[CharacterType::Lowercase],
            "",
            &mut File::open("/dev/urandom").unwrap(),
        );
        let dockerfile = Dockerfile::parse(&format!(
            "ARG BASE_IMAGE=\"{image}\"\nFROM ${{BASE_IMAGE}}\n"
        ))
        .unwrap();

        let actual = dockerfile.base_image(&HashMap::new(), None);

        assert_eq!(actual, Some(image));
    }

    #[test]
    fn when_base_image_with_build_arg_for_arg_in_from_then_returns_the_build_arg() {
        let image = generate_random_string(
            8,
            &[CharacterType::Lowercase],
            "",
            &mut File::open("/dev/urandom").unwrap(),
        );
        let dockerfile = Dockerfile::parse("ARG BASE_IMAGE=default\nFROM ${BASE_IMAGE}\n").unwrap();
        let build_args = HashMap::from([("BASE_IMAGE".to_string(), image.clone())]);

        let actual = dockerfile.base_image(&build_args, None);

        assert_eq!(actual, Some(image));
    }

    #[test]
    fn when_base_image_with_unbraced_arg_in_from_then_returns_the_arg_default() {
        let image = generate_random_string(
            8,
            &[CharacterType::Lowercase],
            "",
            &mut File::open("/dev/urandom").unwrap(),
        );
        let dockerfile =
            Dockerfile::parse(&format!("ARG BASE_IMAGE={image}\nFROM $BASE_IMAGE\n")).unwrap();

        let actual = dockerfile.base_image(&HashMap::new(), None);

        assert_eq!(actual, Some(image));
    }

    #[test]
    fn when_base_image_with_several_args_on_one_line_then_returns_the_args_resolved() {
        let (image, tag) = (
            generate_random_string(
                8,
                &[CharacterType::Lowercase],
                "",
                &mut File::open("/dev/urandom").unwrap(),
            ),
            generate_random_string(
                8,
                &[CharacterType::Lowercase],
                "",
                &mut File::open("/dev/urandom").unwrap(),
            ),
        );
        let dockerfile =
            Dockerfile::parse(&format!("ARG IMAGE={image} TAG={tag}\nFROM $IMAGE:$TAG\n")).unwrap();

        let actual = dockerfile.base_image(&HashMap::new(), None);

        assert_eq!(actual, Some(format!("{image}:{tag}")));
    }

    #[test]
    fn when_base_image_with_unbound_arg_in_from_then_returns_the_image_with_the_arg_empty() {
        let image = generate_random_string(
            8,
            &[CharacterType::Lowercase],
            "",
            &mut File::open("/dev/urandom").unwrap(),
        );
        let dockerfile =
            Dockerfile::parse(&format!("ARG VARIANT\nFROM {image}:${{VARIANT}}\n")).unwrap();

        let actual = dockerfile.base_image(&HashMap::new(), None);

        assert_eq!(actual, Some(format!("{image}:")));
    }

    #[test]
    fn when_base_image_with_arg_referring_to_an_earlier_arg_then_returns_both_resolved() {
        let (image, tag) = (
            generate_random_string(
                8,
                &[CharacterType::Lowercase],
                "",
                &mut File::open("/dev/urandom").unwrap(),
            ),
            generate_random_string(
                8,
                &[CharacterType::Lowercase],
                "",
                &mut File::open("/dev/urandom").unwrap(),
            ),
        );
        let dockerfile = Dockerfile::parse(&format!(
            "ARG TAG={tag}\nARG BASE_IMAGE={image}:${{TAG}}\nFROM ${{BASE_IMAGE}}\n"
        ))
        .unwrap();

        let actual = dockerfile.base_image(&HashMap::new(), None);

        assert_eq!(actual, Some(format!("{image}:{tag}")));
    }

    #[test]
    fn when_base_image_with_arg_declared_after_the_first_from_then_returns_the_default_word() {
        let image = generate_random_string(
            8,
            &[CharacterType::Lowercase],
            "",
            &mut File::open("/dev/urandom").unwrap(),
        );
        let dockerfile = Dockerfile::parse(&format!(
            "FROM build AS build\nARG BASE_IMAGE=other\nFROM ${{BASE_IMAGE:-{image}}}\n"
        ))
        .unwrap();

        let actual = dockerfile.base_image(&HashMap::new(), None);

        assert_eq!(actual, Some(image));
    }

    #[test]
    fn when_base_image_with_target_stage_based_on_another_stage_then_returns_the_image_of_that_stage()
     {
        let image = generate_random_string(
            8,
            &[CharacterType::Lowercase],
            "",
            &mut File::open("/dev/urandom").unwrap(),
        );
        let dockerfile = Dockerfile::parse(&format!(
            "FROM other1 AS stage1\nFROM stage3 AS stage2\nFROM {image} AS stage3\nFROM other4 AS stage4\n"
        ))
        .unwrap();

        let actual = dockerfile.base_image(&HashMap::new(), Some("stage2"));

        assert_eq!(actual, Some(image));
    }

    #[test]
    fn when_base_image_with_last_stage_based_on_another_stage_then_returns_the_image_of_that_stage()
    {
        let image = generate_random_string(
            8,
            &[CharacterType::Lowercase],
            "",
            &mut File::open("/dev/urandom").unwrap(),
        );
        let dockerfile =
            Dockerfile::parse(&format!("FROM {image} AS build\nFROM build\n")).unwrap();

        let actual = dockerfile.base_image(&HashMap::new(), None);

        assert_eq!(actual, Some(image));
    }

    #[test]
    fn when_base_image_with_unknown_target_then_returns_none() {
        let dockerfile = Dockerfile::parse("FROM image AS known\n").unwrap();

        let actual = dockerfile.base_image(&HashMap::new(), Some("unknown"));

        assert_eq!(actual, None);
    }

    #[test]
    fn when_base_image_with_stages_based_on_each_other_then_returns_none() {
        let dockerfile = Dockerfile::parse("FROM b AS a\nFROM a AS b\n").unwrap();

        let actual = dockerfile.base_image(&HashMap::new(), None);

        assert_eq!(actual, None);
    }

    #[test]
    fn when_base_image_without_from_then_returns_none() {
        let dockerfile = Dockerfile::parse("ARG BASE_IMAGE=image\n").unwrap();

        let actual = dockerfile.base_image(&HashMap::new(), None);

        assert_eq!(actual, None);
    }

    #[test]
    fn when_base_image_with_quoted_from_image_then_returns_the_image_without_quotes() {
        let image = generate_random_string(
            8,
            &[CharacterType::Lowercase],
            "",
            &mut File::open("/dev/urandom").unwrap(),
        );
        let dockerfile = Dockerfile::parse(&format!("FROM \"{image}\"\n")).unwrap();

        let actual = dockerfile.base_image(&HashMap::new(), None);

        assert_eq!(actual, Some(image));
    }

    #[test]
    fn when_base_image_with_platform_flag_in_from_then_returns_the_image() {
        let image = generate_random_string(
            8,
            &[CharacterType::Lowercase],
            "",
            &mut File::open("/dev/urandom").unwrap(),
        );
        let dockerfile = Dockerfile::parse(&format!(
            "FROM --platform=$BUILDPLATFORM {image} AS build\n"
        ))
        .unwrap();

        let actual = dockerfile.base_image(&HashMap::new(), None);

        assert_eq!(actual, Some(image));
    }

    #[test]
    fn when_base_image_with_lowercase_indented_instructions_then_returns_the_arg_default() {
        let image = generate_random_string(
            8,
            &[CharacterType::Lowercase],
            "",
            &mut File::open("/dev/urandom").unwrap(),
        );
        let dockerfile = Dockerfile::parse(&format!(
            "  arg BASE_IMAGE={image}\n\tfrom $BASE_IMAGE as base\n"
        ))
        .unwrap();

        let actual = dockerfile.base_image(&HashMap::new(), None);

        assert_eq!(actual, Some(image));
    }

    #[test]
    fn when_base_image_with_alternative_of_set_arg_then_returns_the_word() {
        let (registry, image) = (
            generate_random_string(
                8,
                &[CharacterType::Lowercase],
                "",
                &mut File::open("/dev/urandom").unwrap(),
            ),
            generate_random_string(
                8,
                &[CharacterType::Lowercase],
                "",
                &mut File::open("/dev/urandom").unwrap(),
            ),
        );
        let dockerfile =
            Dockerfile::parse(&format!("ARG cloud\nFROM ${{cloud:+{registry}/}}{image}\n"))
                .unwrap();
        let build_args = HashMap::from([("cloud".to_string(), "true".to_string())]);

        let actual = dockerfile.base_image(&build_args, None);

        assert_eq!(actual, Some(format!("{registry}/{image}")));
    }

    #[test]
    fn when_base_image_with_alternative_of_unset_arg_then_returns_nothing_for_it() {
        let image = generate_random_string(
            8,
            &[CharacterType::Lowercase],
            "",
            &mut File::open("/dev/urandom").unwrap(),
        );
        let dockerfile =
            Dockerfile::parse(&format!("ARG cloud\nFROM ${{cloud:+registry/}}{image}\n")).unwrap();

        let actual = dockerfile.base_image(&HashMap::new(), None);

        assert_eq!(actual, Some(image));
    }

    #[test]
    fn when_base_image_with_default_of_set_arg_then_returns_the_arg() {
        let (registry, image) = (
            generate_random_string(
                8,
                &[CharacterType::Lowercase],
                "",
                &mut File::open("/dev/urandom").unwrap(),
            ),
            generate_random_string(
                8,
                &[CharacterType::Lowercase],
                "",
                &mut File::open("/dev/urandom").unwrap(),
            ),
        );
        let dockerfile =
            Dockerfile::parse(&format!("ARG cloud\nFROM ${{cloud:-default/}}{image}\n")).unwrap();
        let build_args = HashMap::from([("cloud".to_string(), format!("{registry}/"))]);

        let actual = dockerfile.base_image(&build_args, None);

        assert_eq!(actual, Some(format!("{registry}/{image}")));
    }

    #[test]
    fn when_base_image_with_default_of_unset_arg_then_returns_the_word() {
        let (registry, image) = (
            generate_random_string(
                8,
                &[CharacterType::Lowercase],
                "",
                &mut File::open("/dev/urandom").unwrap(),
            ),
            generate_random_string(
                8,
                &[CharacterType::Lowercase],
                "",
                &mut File::open("/dev/urandom").unwrap(),
            ),
        );
        let dockerfile =
            Dockerfile::parse(&format!("ARG cloud\nFROM ${{cloud:-{registry}/}}{image}\n"))
                .unwrap();

        let actual = dockerfile.base_image(&HashMap::new(), None);

        assert_eq!(actual, Some(format!("{registry}/{image}")));
    }

    #[test]
    fn when_base_image_with_quoted_word_of_alternative_then_returns_the_word_without_quotes() {
        let (registry, image) = (
            generate_random_string(
                8,
                &[CharacterType::Lowercase],
                "",
                &mut File::open("/dev/urandom").unwrap(),
            ),
            generate_random_string(
                8,
                &[CharacterType::Lowercase],
                "",
                &mut File::open("/dev/urandom").unwrap(),
            ),
        );
        let dockerfile = Dockerfile::parse(&format!(
            "ARG cloud\nFROM \"${{cloud:+\"{registry}/\"}}{image}\"\n"
        ))
        .unwrap();
        let build_args = HashMap::from([("cloud".to_string(), "true".to_string())]);

        let actual = dockerfile.base_image(&build_args, None);

        assert_eq!(actual, Some(format!("{registry}/{image}")));
    }

    #[test]
    fn when_base_image_with_variable_in_word_of_default_then_returns_the_variable_resolved() {
        let image = generate_random_string(
            8,
            &[CharacterType::Lowercase],
            "",
            &mut File::open("/dev/urandom").unwrap(),
        );
        let dockerfile = Dockerfile::parse(&format!(
            "ARG FALLBACK={image}\nARG BASE_IMAGE\nFROM ${{BASE_IMAGE:-${{FALLBACK}}}}\n"
        ))
        .unwrap();

        let actual = dockerfile.base_image(&HashMap::new(), None);

        assert_eq!(actual, Some(image));
    }

    #[test]
    fn when_base_image_with_target_os_in_from_then_returns_the_image_for_linux() {
        let image = generate_random_string(
            8,
            &[CharacterType::Lowercase],
            "",
            &mut File::open("/dev/urandom").unwrap(),
        );
        let dockerfile = Dockerfile::parse(&format!("FROM {image}-${{TARGETOS}}\n")).unwrap();

        let actual = dockerfile.base_image(&HashMap::new(), None);

        assert_eq!(actual, Some(format!("{image}-linux")));
    }

    #[test]
    fn when_parse_with_from_without_image_then_returns_none() {
        let text = "FROM\n";

        let actual = Dockerfile::parse(text);

        assert!(actual.is_none());
    }

    #[test]
    fn when_parse_with_unclosed_braced_variable_then_returns_none() {
        let text = "ARG BASE_IMAGE=image\nFROM ${BASE_IMAGE\n";

        let actual = Dockerfile::parse(text);

        assert!(actual.is_none());
    }
}
