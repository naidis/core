use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Table {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub alignments: Vec<Alignment>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Alignment {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseTableRequest {
    pub markdown: String,
}

pub fn parse_table(request: &ParseTableRequest) -> Result<Table> {
    let lines: Vec<&str> = request.markdown.trim().lines().collect();

    if lines.len() < 2 {
        anyhow::bail!("Invalid table: need at least header and separator rows");
    }

    let headers = parse_row(lines[0]);
    let alignments = parse_alignments(lines[1], headers.len());

    let rows: Vec<Vec<String>> = lines[2..].iter().map(|line| parse_row(line)).collect();

    Ok(Table {
        headers,
        rows,
        alignments,
    })
}

fn parse_row(line: &str) -> Vec<String> {
    let line = line.trim().trim_start_matches('|').trim_end_matches('|');
    line.split('|')
        .map(|cell| cell.trim().to_string())
        .collect()
}

fn parse_alignments(line: &str, count: usize) -> Vec<Alignment> {
    let line = line.trim().trim_start_matches('|').trim_end_matches('|');
    let cells: Vec<&str> = line.split('|').collect();

    let mut alignments = Vec::new();
    for cell in cells.iter().take(count) {
        let cell = cell.trim();
        let align = if cell.starts_with(':') && cell.ends_with(':') {
            Alignment::Center
        } else if cell.ends_with(':') {
            Alignment::Right
        } else {
            Alignment::Left
        };
        alignments.push(align);
    }

    while alignments.len() < count {
        alignments.push(Alignment::Left);
    }

    alignments
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatTableRequest {
    pub table: Table,
    pub pretty: Option<bool>,
}

pub fn format_table(request: &FormatTableRequest) -> Result<String> {
    let table = &request.table;
    let pretty = request.pretty.unwrap_or(true);

    if pretty {
        format_pretty_table(table)
    } else {
        format_compact_table(table)
    }
}

fn format_pretty_table(table: &Table) -> Result<String> {
    let _col_count = table.headers.len();
    let mut col_widths: Vec<usize> = table.headers.iter().map(|h| h.len()).collect();

    for row in &table.rows {
        for (i, cell) in row.iter().enumerate() {
            if i < col_widths.len() && cell.len() > col_widths[i] {
                col_widths[i] = cell.len();
            }
        }
    }

    let mut output = String::new();

    output.push('|');
    for (i, header) in table.headers.iter().enumerate() {
        let width = col_widths.get(i).copied().unwrap_or(3);
        output.push_str(&format!(" {:width$} |", header, width = width));
    }
    output.push('\n');

    output.push('|');
    for (i, align) in table.alignments.iter().enumerate() {
        let width = col_widths.get(i).copied().unwrap_or(3);
        let sep = match align {
            Alignment::Left => format!(":{:-<width$}|", "", width = width + 1),
            Alignment::Center => format!(":{:-^width$}:|", "", width = width),
            Alignment::Right => format!("{:-<width$}:|", "", width = width + 1),
        };
        output.push_str(&sep);
    }
    output.push('\n');

    for row in &table.rows {
        output.push('|');
        for (i, cell) in row.iter().enumerate() {
            let width = col_widths.get(i).copied().unwrap_or(3);
            let align = table.alignments.get(i).unwrap_or(&Alignment::Left);
            let formatted = match align {
                Alignment::Left => format!(" {:<width$} |", cell, width = width),
                Alignment::Center => format!(" {:^width$} |", cell, width = width),
                Alignment::Right => format!(" {:>width$} |", cell, width = width),
            };
            output.push_str(&formatted);
        }
        output.push('\n');
    }

    Ok(output.trim_end().to_string())
}

fn format_compact_table(table: &Table) -> Result<String> {
    let mut output = String::new();

    output.push_str("| ");
    output.push_str(&table.headers.join(" | "));
    output.push_str(" |\n");

    output.push('|');
    for align in &table.alignments {
        let sep = match align {
            Alignment::Left => ":---|",
            Alignment::Center => ":---:|",
            Alignment::Right => "---:|",
        };
        output.push_str(sep);
    }
    output.push('\n');

    for row in &table.rows {
        output.push_str("| ");
        output.push_str(&row.join(" | "));
        output.push_str(" |\n");
    }

    Ok(output.trim_end().to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortTableRequest {
    pub table: Table,
    pub column: usize,
    pub descending: Option<bool>,
    pub numeric: Option<bool>,
}

pub fn sort_table(request: &SortTableRequest) -> Result<Table> {
    let mut table = request.table.clone();
    let col = request.column;
    let desc = request.descending.unwrap_or(false);
    let numeric = request.numeric.unwrap_or(false);

    if col >= table.headers.len() {
        anyhow::bail!("Column index out of bounds");
    }

    table.rows.sort_by(|a, b| {
        let a_val = a.get(col).map(|s| s.as_str()).unwrap_or("");
        let b_val = b.get(col).map(|s| s.as_str()).unwrap_or("");

        let cmp = if numeric {
            let a_num: f64 = a_val.parse().unwrap_or(0.0);
            let b_num: f64 = b_val.parse().unwrap_or(0.0);
            a_num
                .partial_cmp(&b_num)
                .unwrap_or(std::cmp::Ordering::Equal)
        } else {
            a_val.cmp(b_val)
        };

        if desc {
            cmp.reverse()
        } else {
            cmp
        }
    });

    Ok(table)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddRowRequest {
    pub table: Table,
    pub row: Vec<String>,
    pub position: Option<usize>,
}

pub fn add_row(request: &AddRowRequest) -> Result<Table> {
    let mut table = request.table.clone();
    let mut row = request.row.clone();

    while row.len() < table.headers.len() {
        row.push(String::new());
    }
    row.truncate(table.headers.len());

    if let Some(pos) = request.position {
        if pos <= table.rows.len() {
            table.rows.insert(pos, row);
        } else {
            table.rows.push(row);
        }
    } else {
        table.rows.push(row);
    }

    Ok(table)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddColumnRequest {
    pub table: Table,
    pub header: String,
    pub default_value: Option<String>,
    pub position: Option<usize>,
}

pub fn add_column(request: &AddColumnRequest) -> Result<Table> {
    let mut table = request.table.clone();
    let default = request.default_value.clone().unwrap_or_default();
    let pos = request.position.unwrap_or(table.headers.len());

    table
        .headers
        .insert(pos.min(table.headers.len()), request.header.clone());
    table
        .alignments
        .insert(pos.min(table.alignments.len()), Alignment::Left);

    for row in &mut table.rows {
        row.insert(pos.min(row.len()), default.clone());
    }

    Ok(table)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteRowRequest {
    pub table: Table,
    pub row_index: usize,
}

pub fn delete_row(request: &DeleteRowRequest) -> Result<Table> {
    let mut table = request.table.clone();
    if request.row_index < table.rows.len() {
        table.rows.remove(request.row_index);
    }
    Ok(table)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteColumnRequest {
    pub table: Table,
    pub column_index: usize,
}

pub fn delete_column(request: &DeleteColumnRequest) -> Result<Table> {
    let mut table = request.table.clone();
    let col = request.column_index;

    if col < table.headers.len() {
        table.headers.remove(col);
        table.alignments.remove(col.min(table.alignments.len() - 1));
        for row in &mut table.rows {
            if col < row.len() {
                row.remove(col);
            }
        }
    }

    Ok(table)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveColumnRequest {
    pub table: Table,
    pub from_index: usize,
    pub to_index: usize,
}

pub fn move_column(request: &MoveColumnRequest) -> Result<Table> {
    let mut table = request.table.clone();
    let from = request.from_index;
    let to = request.to_index;

    if from >= table.headers.len() || to >= table.headers.len() {
        anyhow::bail!("Column index out of bounds");
    }

    let header = table.headers.remove(from);
    table.headers.insert(to, header);

    let align = table.alignments.remove(from);
    table.alignments.insert(to, align);

    for row in &mut table.rows {
        if from < row.len() {
            let cell = row.remove(from);
            row.insert(to.min(row.len()), cell);
        }
    }

    Ok(table)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetAlignmentRequest {
    pub table: Table,
    pub column_index: usize,
    pub alignment: String,
}

pub fn set_alignment(request: &SetAlignmentRequest) -> Result<Table> {
    let mut table = request.table.clone();
    let col = request.column_index;

    if col >= table.alignments.len() {
        anyhow::bail!("Column index out of bounds");
    }

    table.alignments[col] = match request.alignment.to_lowercase().as_str() {
        "left" | "l" => Alignment::Left,
        "center" | "c" => Alignment::Center,
        "right" | "r" => Alignment::Right,
        _ => Alignment::Left,
    };

    Ok(table)
}
