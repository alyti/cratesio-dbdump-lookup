# cratesio-dbdump-lookup

Utility trait for rusqlite::Connection for working with data created by cratesio-dbdump-csvtab.

Fetch from crates.io experimental data dump to adhere to their [data-access policies](https://crates.io/data-access).

## Notice
Running the example `default` will download and unpack to around 500 mb of data.