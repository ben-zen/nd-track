# nd-track: CLI national debt tracker

As an experiment into building tools using nom-xml & nom-xml-derive, I decided to
first build a tool to read a simple Treasury API first, the national debt to the
penny dataset. I'm going to start with a utility that just grabs the past couple
of days, outputs today's data, and compares it to the day before.

From there, it's a matter of adding filters, and eventually developing a TUI-based
interactive mode.

## Current command line interface

```
> $ nd-track --help
Usage:
  nd-track [OPTIONS]

Reads the U.S. Treasury "Debt to the Penny" API for a span of time.

Optional arguments:
  -h,--help             Show this help message and exit
  -d,--date DATE        Starting date for the lookup in YYYY-MM-DD format;
                        defaults to 2026-07-22.
```

## dev log

I started out trying to use `nom-xml-derive`, but about the point where I wanted to
use it with `Money<USD>` I ran into fundamental incompatibilities, so I've been forced
to abandon that library and write my own. Notes about that are below.

I jumped to a builder pattern because that's served me well in the past with parsing
in Rust, especially when handling free-form data. It's relatively light-weight, and could feasibly
be used on a streaming interface instead of the all-in-memory approach of `nom`, which may become
an issue when handling large data sets.

For testing purposes, although I haven't implemented local file reading yet, I do have a sample
server output to work with. In order to avoid hammering the server, I'm going to add caching and
local result logging. I'll have to figure out handling date ranges, and restructuring data.

## builder_derive

I'm thinking about ways to approach automating transforming data sets into structures. Ideally,
I'd be able to provide element names as attributes on struct fields, like so:
```rust
#[derive(Builder,XmlBuilder)]
struct Date {
    #[element("record_calendar_year")]
    year: u16,
    #[element("record_calendar_quarter")]
    quarter: u8,
    #[element("record_calendar_month")]
    month: u8,
    #[element("record_calendar_day")]
    day: u8
}
```

In this example, `XmlBuilder` uses the helpers provided by the `Builder` pattern derivation.

While I refine my ideas for this, I'm just implementing it in full form, so I can find the most refined
version to use in template form (and also learn how to write a macro for this.)
