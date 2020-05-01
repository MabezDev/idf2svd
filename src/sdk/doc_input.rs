use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct JSONInput {
    data: Vec<Vec<Column>>,
}

#[derive(Debug, Deserialize)]
struct Column {
    text: String,
}

#[derive(Debug, Deserialize)]
#[serde(from = "Vec<JSONInput>")]
pub struct Table {
    pub header: Vec<String>,
    pub data: Vec<Vec<String>>,
}

impl From<Vec<JSONInput>> for Table {
    fn from(mut input: Vec<JSONInput>) -> Self {
        let input = input.pop().unwrap();
        let mut data = input.data.into_iter().skip(1); // first row is garbage

        let header = loop {
            let header = data.next().unwrap();
            if !header[4].text.is_empty() {
                break header
                    .into_iter()
                    .map(|c| c.text.replace("\r", ""))
                    .collect();
            }
        };

        Table {
            header,
            data: data
                .map(|row| row.into_iter().map(|c| c.text).collect())
                .collect(),
        }
    }
}
