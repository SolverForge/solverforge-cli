#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CountableRange {
    pub from: usize,
    pub to: usize,
}

impl CountableRange {
    pub(crate) fn parse(raw: &str) -> Result<Self, String> {
        let (from, to) = raw.split_once("..").ok_or_else(|| {
            "countable range must use `from..to` syntax with non-negative integers".to_string()
        })?;
        if to.contains("..") {
            return Err(
                "countable range must use `from..to` syntax with non-negative integers".to_string(),
            );
        }
        let from = from
            .trim()
            .parse::<usize>()
            .map_err(|_| "countable range start must be a non-negative integer".to_string())?;
        let to = to
            .trim()
            .parse::<usize>()
            .map_err(|_| "countable range end must be a non-negative integer".to_string())?;
        if from >= to {
            return Err("countable range must satisfy from < to".to_string());
        }
        if i64::try_from(from).is_err() || i64::try_from(to).is_err() {
            return Err("countable range bounds must fit in a signed 64-bit integer".to_string());
        }
        Ok(Self { from, to })
    }
}

#[cfg(test)]
mod tests {
    use super::CountableRange;

    #[test]
    fn parses_half_open_non_negative_range() {
        assert_eq!(
            CountableRange::parse("2..7"),
            Ok(CountableRange { from: 2, to: 7 })
        );
    }

    #[test]
    fn rejects_invalid_and_empty_ranges() {
        assert!(CountableRange::parse("-1..3").is_err());
        assert!(CountableRange::parse("3..3").is_err());
        assert!(CountableRange::parse("4..2").is_err());
        assert!(CountableRange::parse("1...3").is_err());
    }
}
