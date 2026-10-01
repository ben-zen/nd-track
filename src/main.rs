// SPDX-FileCopyrightText: Copyright Ben Lewis, 2026.
// SPDX-License-Identifier: Artistic-2.0

use std::time::Duration;
use std::{boxed, fmt};

use anyhow::{Result, anyhow, bail};
use argparse::{ArgumentParser, Store};
use doubloon::Money;
use doubloon::iso_currencies::USD;
use nom::bytes::complete::take_while_m_n;
use nom::{character::complete::char, combinator::eof, sequence::tuple};
use nom_xml::{Document, config::Config, parse::Parse, tag::Tag};
use reqwest;
use rust_decimal::Decimal;
use tracing::Instrument;

fn dec_string_to_money(value: &str) -> Result<Money<USD>> {
    Ok(Money::new(Decimal::from_str_exact(value)?, USD))
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Date {
    // record_calendar_year
    // #[extract(from_tag = "record_calendar_year")]
    year: u16,
    // #[extract(from_tag = "record_calendar_quarter")]
    quarter: u8,
    // #[extract(from_tag = "record_calendar_month")]
    month: u8,
    // #[extract(from_tag = "record_calendar_day")]
    day: u8,
}

impl std::str::FromStr for Date {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        // expect YYYY-MM-DD format
        let year = nom::bytes::complete::take_while_m_n(4, 4, |c: char| c.is_digit(10));
        let month = nom::bytes::complete::take_while_m_n(2, 2, |c: char| c.is_digit(10));
        let day = nom::bytes::complete::take_while_m_n(2, 2, |c: char| c.is_digit(10));

        let (_, (year, _, month, _, day, _)) = tuple::<_, _, nom::error::Error<&str>, _>((year, nom::character::complete::char('-'), month, nom::character::complete::char('-'), day, eof))(s).map_err(|e| anyhow!("dates should be in the format YYYY-MM-DD: {:?}", e))?;

        let year: u16 = year.parse()?;
        let month: u8 = month.parse()?;
        let day: u8 = day.parse()?;

        Date::try_init(year, month, day)
    }
}

impl fmt::Display for Date {
    // TODO: support alternate formats
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day) // YYYY-MM-DD
    }
}

impl Date {
    fn try_init(year: u16, month: u8, day: u8) -> Result<Self> {
        // validate year
        let quarter = match month {
            1..=3 => 1,
            4..=6 => 2,
            7..=9 => 3,
            10..=12 => 4,
            _ => bail!("couldn't capture a quarter for month {month}"),
        };

        Ok(Date {
            year,
            quarter,
            month,
            day,
        })
    }
}

#[derive(Debug, Default)]
struct DateBuilder {
    year: Option<u16>,
    quarter: Option<u8>,
    month: Option<u8>,
    day: Option<u8>,
}

impl DateBuilder {
    fn set_year(mut self, year: u16) -> Result<Self> {
        match self.year {
            Some(y) => Err(anyhow!("Pre-existing year: {y}")),
            None => {
                self.year = Some(year);
                Ok(self)
            }
        }
    }

    fn set_quarter(mut self, quarter: u8) -> Result<Self> {
        match self.quarter {
            Some(q) => Err(anyhow!("quarter already populated: {q}")),
            None => {
                self.quarter = Some(quarter);
                Ok(self)
            }
        }
    }

    fn set_month(mut self, month: u8) -> Result<Self> {
        match self.month {
            Some(m) => Err(anyhow!("month already populated: {m}")),
            None => {
                self.month = Some(month);
                Ok(self)
            }
        }
    }

    fn set_day(mut self, day: u8) -> Result<Self> {
        match self.day {
            Some(d) => Err(anyhow!("month already populated: {d}")),
            None => {
                self.day = Some(day);
                Ok(self)
            }
        }
    }

    fn incomplete(&self) -> bool {
        self.year.is_none() || self.quarter.is_none() || self.month.is_none() || self.day.is_none()
    }

    fn build(self) -> Result<Date> {
        Ok(Date {
            year: self.year.ok_or_else(|| anyhow!("missing year"))?,
            quarter: self.quarter.ok_or_else(|| anyhow!("missing quarter"))?,
            month: self.month.ok_or_else(|| anyhow!("missing month"))?,
            day: self.day.ok_or_else(|| anyhow!("missing day"))?,
        })
    }
}

impl DateBuilder {
    // Extract from XML. Reports if the element is consumed.
    fn read_from_element(self, tag: &str, content: &Document) -> Result<(Self, bool)> {
        if !self.incomplete() {
            return Ok((self, false));
        };

        tracing::trace!("reading {tag}, {content:?}");
        let Document::Content(Some(c)) = content else {
            tracing::info!("unexpected non-content doc element: {content:?}");
            return Ok((self, false));
        };

        match tag {
            "record_calendar_year" => Ok((self.set_year(c.parse()?)?, true)),
            "record_calendar_quarter" => Ok((self.set_quarter(c.parse()?)?, true)),
            "record_calendar_month" => Ok((self.set_month(c.parse()?)?, true)),
            "record_calendar_day" => Ok((self.set_day(c.parse()?)?, true)),
            _ => {
                tracing::trace!("couldn't parse {tag}");
                Ok((self, false))
            }
        }
    }

    // Write an overall XML extractor function for Document via a Builder
    fn build_from_document(doc: &Document) -> Result<Date> {
        let Document::Element(_, inner_box, _) = doc else {
            bail!("expected Document::Element")
        };

        let inner = inner_box.as_ref();

        let Document::Nested(elems) = inner else {
            bail!("bad extraction");
        };

        elems
            .iter()
            .filter_map(|elem| {
                if let Document::Element(tag, inner, _) = elem {
                    Some((tag, inner))
                } else {
                    None
                }
            })
            .try_fold(
                DateBuilder::default(),
                |builder, (tag, inner)| -> Result<DateBuilder> {
                    let (build, _consumed) =
                        builder.read_from_element(tag.name.local_part.as_str(), &inner)?;
                    //assert!(consumed);
                    Ok(build)
                },
            )?
            .build()
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
struct FiscalDate {
    // #[extract(from_tag = "record_fiscal_year")]
    year: u16,
    // #[extract(from_tag = "record_fiscal_quarter")]
    quarter: u8,
}

#[derive(Default)]
struct FiscalDateBuilder {
    year: Option<u16>,
    quarter: Option<u8>,
}

// Builder impl
impl FiscalDateBuilder {
    fn set_year(mut self, year: u16) -> Result<Self> {
        match self.year {
            Some(y) => Err(anyhow!("year already populated: {y}")),
            None => {
                self.year = Some(year);
                Ok(self)
            }
        }
    }

    fn set_quarter(mut self, quarter: u8) -> Result<Self> {
        match self.quarter {
            Some(q) => Err(anyhow!("quarter already populated: {q}")),
            None => {
                self.quarter = Some(quarter);
                Ok(self)
            }
        }
    }

    fn build(self) -> Result<FiscalDate> {
        Ok(FiscalDate {
            year: self.year.ok_or_else(|| anyhow!("missing year"))?,
            quarter: self.quarter.ok_or_else(|| anyhow!("missing quarter"))?,
        })
    }
}

// XML api
impl FiscalDateBuilder {
    /// Takes the tag-name of an actively read Document::Element, and the Document::Content of it as a &Document.
    /// It is an error to pass anything else as the &Document argument.
    fn read_from_element(self, tag: &str, content: &Document) -> Result<(Self, bool)> {
        let Document::Content(Some(c)) = content else {
            tracing::info!("unexpected non-content doc element: {content:?}");
            return Ok((self, false));
        };

        match tag {
            "record_fiscal_year" => Ok((self.set_year(c.parse()?)?, true)),
            "record_fiscal_quarter" => Ok((self.set_quarter(c.parse()?)?, true)),
            _ => {
                tracing::trace!("couldn't parse {tag}");
                Ok((self, false))
            }
        }
    }

    // Write an overall XML extractor function for Document via a Builder
    fn build_from_document(doc: &Document) -> Result<FiscalDate> {
        let Document::Element(_, inner_box, _) = doc else {
            bail!("expected Document::Element")
        };

        let inner = inner_box.as_ref();

        let Document::Nested(elems) = inner else {
            bail!("bad extraction");
        };

        elems
            .iter()
            .filter_map(|elem| {
                if let Document::Element(tag, inner, _) = elem {
                    Some((tag, inner))
                } else {
                    None
                }
            })
            .try_fold(
                FiscalDateBuilder::default(),
                |builder, (tag, inner)| -> Result<FiscalDateBuilder> {
                    let (build, _consumed) =
                        builder.read_from_element(tag.name.local_part.as_str(), &inner)?;
                    //assert!(consumed);
                    Ok(build)
                },
            )?
            .build()
    }
}

// data-element
#[derive(Debug, Clone, PartialEq)]
struct LedgerEntry {
    // Record date
    // DATE
    // fill from fields
    date: Date,

    // Record fiscal date
    // DATE
    // fill from fields
    fiscal_date: FiscalDate,

    // meta/labels/tot_pub_debt_out_amt ("Debt Held by the Public")
    // meta/dataTypes/debt_held_public_amt CURRENCY
    // #[extract(from_tag = "debt_held_public_amt")]
    public_debt: Money<USD>,

    // meta/labels/intragov_hold_amt ("Intragovernmental Holdings")
    // meta/dataTypes/intragov_hold_amt CURRENCY
    // #[extract(from_tag = "intragov_hold_amt")]
    intragov_debt: Money<USD>,

    // meta/labels/tot_pub_debt_out_amt ("Total Public Debt Outstanding")
    // meta/dataTypes/tot_pub_debt_out_amt CURRENCY
    // #[extract(from_tag = "tot_pub_debt_out_amt")]
    total_debt: Money<USD>,
}

#[derive(Default)]
struct LedgerEntryBuilder {
    date: DateBuilder,
    fiscal_date: FiscalDateBuilder,
    public_debt: Option<Money<USD>>,
    intragov_debt: Option<Money<USD>>,
    total_debt: Option<Money<USD>>,
}

// Builder impl
impl LedgerEntryBuilder {
    fn set_public_debt(mut self, public_debt: Money<USD>) -> Result<Self> {
        match self.public_debt {
            Some(pd) => Err(anyhow!("public_debt already populated: {pd}")),
            None => {
                self.public_debt = Some(public_debt);
                Ok(self)
            }
        }
    }

    fn set_intragov_debt(mut self, intragov_debt: Money<USD>) -> Result<Self> {
        match self.intragov_debt {
            Some(igd) => Err(anyhow!("intragov_debt already populated: {igd}")),
            None => {
                self.intragov_debt = Some(intragov_debt);
                Ok(self)
            }
        }
    }

    fn set_total_debt(mut self, total_debt: Money<USD>) -> Result<Self> {
        match self.total_debt {
            Some(td) => Err(anyhow!("total_debt already populated: {td}")),
            None => {
                self.total_debt = Some(total_debt);
                Ok(self)
            }
        }
    }

    fn build(self) -> Result<LedgerEntry> {
        Ok(LedgerEntry {
            date: self.date.build()?,
            fiscal_date: self.fiscal_date.build()?,
            public_debt: self
                .public_debt
                .ok_or_else(|| anyhow!("missing public_debt"))?,
            intragov_debt: self
                .intragov_debt
                .ok_or_else(|| anyhow!("missing intragov_debt"))?,
            total_debt: self
                .total_debt
                .ok_or_else(|| anyhow!("missing total_debt"))?,
        })
    }
}

// XML extracting builder
impl LedgerEntryBuilder {
    fn read_from_element(mut self, tag: &str, content: &Document) -> Result<(Self, bool)> {
        // visit each of the sub-builders first. I feel like this is fragile but it's a start.'
        let (date, consumed) = self.date.read_from_element(tag, content)?;
        self.date = date;
        if consumed {
            return Ok((self, true));
        }

        let (fiscal_date, consumed) = self.fiscal_date.read_from_element(tag, content)?;
        self.fiscal_date = fiscal_date;
        if consumed {
            return Ok((self, true));
        }

        let Document::Content(Some(c)) = content else {
            bail!("encountered an unsupported doc element {content:?}");
        };

        match tag {
            "debt_held_public_amt" => Ok((self.set_public_debt(dec_string_to_money(c)?)?, true)),
            "intragov_hold_amt" => Ok((self.set_intragov_debt(dec_string_to_money(c)?)?, true)),
            "tot_pub_debt_out_amt" => Ok((self.set_total_debt(dec_string_to_money(c)?)?, true)),
            _ => {
                tracing::trace!("ignored {tag}");
                Ok((self, false))
            }
        }
    }

    // Write an overall XML extractor function for Document via a Builder
    fn build_from_document(doc: &Document) -> Result<LedgerEntry> {
        let Document::Element(_, inner_box, _) = doc else {
            bail!("expected Document::Element")
        };

        let inner = inner_box.as_ref();

        let Document::Nested(elems) = inner else {
            bail!("bad extraction");
        };

        elems
            .iter()
            .filter_map(|elem| {
                if let Document::Element(tag, inner, _) = elem {
                    Some((tag, inner))
                } else {
                    None
                }
            })
            .try_fold(
                LedgerEntryBuilder::default(),
                |builder, (tag, inner)| -> Result<LedgerEntryBuilder> {
                    let (build, _consumed) =
                        builder.read_from_element(tag.name.local_part.as_str(), &inner)?;
                    //assert!(consumed);
                    Ok(build)
                },
            )?
            .build()
    }
}

static APP_USER_AGENT: &str = "nd-track/devel";
static DEFAULT_LOOKUP_DATE: &str = "2026-07-22";

fn main() -> Result<()> {
    tracing_subscriber::fmt().init();
    let mut date = String::from(DEFAULT_LOOKUP_DATE);
    {
        let mut ap = ArgumentParser::new();
        ap.set_description(r#"Reads the U.S. Treasury "Debt to the Penny" API for a span of time."#);
        ap.refer(&mut date).add_option(&["-d", "--date"], Store, "Starting date for the lookup in YYYY-MM-DD format; defaults to 2026-07-22.");
        ap.parse_args_or_exit();
    }

    let start_date: Date = date.parse().expect("This is an expected date.");

    let lookup = std::format!(
        r#"https://api.fiscaldata.treasury.gov/services/api/fiscal_service/v2/accounting/od/debt_to_penny?filter=record_date:gte:{start_date}&format=xml"#
    );
    let client = reqwest::blocking::Client::builder()
        .user_agent(APP_USER_AGENT)
        .timeout(Duration::from_secs(60))
        .build()?;

    let lookup_span = tracing::info_span!("requesting fiscal data");
    let ls_guard = lookup_span.enter();
    let result = client.get(lookup).send()?.error_for_status()?;
    let response_doc = result.text()?;
    drop(ls_guard);

    let parsing_span = tracing::info_span!("parsing data");
    let ps_guard = parsing_span.enter();

    let (_, records) =
        Document::parse_elements_by_tag_name(response_doc.as_str(), "data-element", &None)?;
    let (_, links) = Document::parse_element_by_tag_name(response_doc.as_str(), "links", &None)?;
    let (_, _labels) = Document::parse_element_by_tag_name(response_doc.as_str(), "labels", &None)?;

    drop(ps_guard);

    // This should get fixed eventually...
    let debt_records = records
        .iter()
        .map(LedgerEntryBuilder::build_from_document)
        .map(|entry| entry.unwrap());

    println!("Received debt records:");
    debt_records.for_each(|r| println!("{r:?}"));

    println!("Pagination:\n{links:?}");
    /*println!("Labels:\n{labels:?}");*/

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::bail;
    use nom_xml::Document;

    #[test]
    fn test_date_init() -> Result<()> {
        Date::try_init(2026, 28, 9)
            .map_or_else(|_| Ok(()), |d| bail!("Expected failure, not {d:?}"))
            .unwrap();
        let demo_date = Date::try_init(2026, 9, 28)
            .or_else(|e| bail!("expected success, not {e:?}"))
            .unwrap();
        assert_eq!(demo_date.to_string(), String::from("2026-09-28"));

        assert_eq!(Date::try_init(2026, 9, 28).unwrap(), "2026-09-28".parse()?);

        Ok(())
    }

    #[test]
    fn test_build_date() -> Result<()> {
        let builder = DateBuilder::default();

        let built_date = builder
            .set_year(2026)?
            .set_quarter(3)?
            .set_month(8)?
            .set_day(27)?
            .build()?;

        assert_eq!(built_date.year, 2026);
        assert_eq!(built_date.quarter, 3);
        assert_eq!(built_date.month, 8);
        assert_eq!(built_date.day, 27);

        Ok(())
    }

    #[test]
    fn test_xml_build_date() -> Result<()> {
        let sample_record = r#"
<data-element>
    <record_date>2026-08-27</record_date>
    <debt_held_public_amt>32313802811901.63</debt_held_public_amt>
    <intragov_hold_amt>7763727020041.31</intragov_hold_amt>
    <tot_pub_debt_out_amt>40077529831942.94</tot_pub_debt_out_amt>
    <src_line_nbr>1</src_line_nbr>
    <record_fiscal_year>2026</record_fiscal_year>
    <record_fiscal_quarter>4</record_fiscal_quarter>
    <record_calendar_year>2026</record_calendar_year>
    <record_calendar_quarter>3</record_calendar_quarter>
    <record_calendar_month>08</record_calendar_month>
    <record_calendar_day>27</record_calendar_day>
</data-element>
)"#;
        let (_, doc) = Document::parse_element_by_tag_name(sample_record, "data-element", &None)?;

        let built_date = DateBuilder::build_from_document(&doc)?;

        assert_eq!(built_date.year, 2026);
        assert_eq!(built_date.quarter, 3);
        assert_eq!(built_date.month, 8);
        assert_eq!(built_date.day, 27);

        Ok(())
    }

    #[test]
    fn test_build_fiscal_date() -> Result<()> {
        let builder = FiscalDateBuilder::default();

        let built_fiscal_date = builder.set_year(2026)?.set_quarter(3)?.build()?;

        assert_eq!(built_fiscal_date.year, 2026);
        assert_eq!(built_fiscal_date.quarter, 3);

        Ok(())
    }

    #[test]
    fn test_xml_build_fiscal_date() -> Result<()> {
        let sample_record = r#"
        <data-element>
        <record_date>2026-08-27</record_date>
        <debt_held_public_amt>32313802811901.63</debt_held_public_amt>
        <intragov_hold_amt>7763727020041.31</intragov_hold_amt>
        <tot_pub_debt_out_amt>40077529831942.94</tot_pub_debt_out_amt>
        <src_line_nbr>1</src_line_nbr>
        <record_fiscal_year>2026</record_fiscal_year>
        <record_fiscal_quarter>4</record_fiscal_quarter>
        <record_calendar_year>2026</record_calendar_year>
        <record_calendar_quarter>3</record_calendar_quarter>
        <record_calendar_month>08</record_calendar_month>
        <record_calendar_day>27</record_calendar_day>
        </data-element>
        )"#;
        let (_, doc) = Document::parse_element_by_tag_name(sample_record, "data-element", &None)?;

        let built_date = FiscalDateBuilder::build_from_document(&doc)?;

        assert_eq!(built_date.year, 2026);
        assert_eq!(built_date.quarter, 4);
        Ok(())
    }

    #[test]
    fn test_xml_build_ledger_entry() -> Result<()> {
        let sample_record = r#"
        <data-element>
        <record_date>2026-08-27</record_date>
        <debt_held_public_amt>32313802811901.63</debt_held_public_amt>
        <intragov_hold_amt>7763727020041.31</intragov_hold_amt>
        <tot_pub_debt_out_amt>40077529831942.94</tot_pub_debt_out_amt>
        <src_line_nbr>1</src_line_nbr>
        <record_fiscal_year>2026</record_fiscal_year>
        <record_fiscal_quarter>4</record_fiscal_quarter>
        <record_calendar_year>2026</record_calendar_year>
        <record_calendar_quarter>3</record_calendar_quarter>
        <record_calendar_month>08</record_calendar_month>
        <record_calendar_day>27</record_calendar_day>
        </data-element>
        )"#;
        let (_, doc) = Document::parse_element_by_tag_name(sample_record, "data-element", &None)?;

        let built_ledger_entry = LedgerEntryBuilder::build_from_document(&doc)?;

        assert_eq!(built_ledger_entry.date.year, 2026);
        assert_eq!(built_ledger_entry.fiscal_date.quarter, 4);
        assert_eq!(
            built_ledger_entry.total_debt,
            Money::from_minor_units(4007752983194294, USD)
        );
        Ok(())
    }
}
