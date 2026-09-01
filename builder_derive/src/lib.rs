// SPDX-FileCopyrightText: Copyright Ben Lewis, 2026.
// SPDX-License-Identifier: Artistic-2.0

use proc_macro::TokenStream;
use quote::quote;

/// The builder trait defines a companion *Builder struct with Option<> fields
/// to match the main struct. Each field gets a function
/*
 * #[derive(Debug, Default, Clone, PartialEq)]
 *  s *truct Date {
 *  // record_calendar_year
 *  // #[extract(from_tag = "record_calendar_year")]
 *  year: u16,
 *  // #[extract(from_tag = "record_calendar_quarter")]
 *  quarter: u8,
 *  // #[extract(from_tag = "record_calendar_month")]
 *  month: u8,
 *  // #[extract(from_tag = "record_calendar_day")]
 *  day: u8
 * }
 *
 * #[derive(Debug, Default)]
 * struct DateBuilder {
 * year: Option<u16>,
 * quarter: Option<u8>,
 * month: Option<u8>,
 * day: Option<u8>
 * }
 *
 * impl DateBuilder {
 * fn set_year(mut self, year: u16) -> Result<Self> {
 * match self.year {
 * Some(y) => Err(anyhow!("Pre-existing year: {y}")),
 * None => {
 * self.year = Some(year);
 * Ok(self)
 * },
 * }
 * }
 *
 * fn set_quarter(mut self, quarter: u8) -> Result<Self> {
 * match self.quarter {
 * Some(q) => Err(anyhow!("quarter already populated: {q}")),
 * None => {
 * self.quarter = Some(quarter);
 * Ok(self)
 * },
 * }
 * }
 *
 * fn set_month(mut self, month: u8) -> Result<Self> {
 * match self.month {
 * Some(m) => Err(anyhow!("month already populated: {m}")),
 * None => {
 * self.month = Some(month);
 * Ok(self)
 * },
 * }
 * }
 *
 * fn set_day(mut self, day: u8) -> Result<Self> {
 * match self.day {
 * Some(d) => Err(anyhow!("month already populated: {d}")),
 * None => {
 * self.day = Some(day);
 * Ok(self)
 * },
 * }
 * }
 *
 * fn build(self) -> Result<Date> {
 * Ok(Date {
 * year: self.year.ok_or_else(|| { anyhow!("missing year") })?,
 * quarter: self.quarter.ok_or_else(|| { anyhow!("missing quarter") })?,
 * month: self.month.ok_or_else(|| { anyhow!("missing month") })?,
 * day: self.day.ok_or_else(|| { anyhow!("missing day") })?
 * })
 * }
 * }
*/

fn builder_impl(ast: &DeriveInput) -> TokenStream {
    unimplemented!()

}


#[proc_macro_derive(Builder)]
pub fn builder_derive(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();

    builder_impl()
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
