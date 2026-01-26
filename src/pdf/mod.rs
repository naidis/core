use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

use crate::rpc::{PdfMetadata, PdfRequest, PdfResponse};

static DEPS_CHECKED: OnceLock<DependencyStatus> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct DependencyStatus {
    pub tesseract: bool,
    pub pdftoppm: bool,
    pub magick: bool,
    pub tabula: bool,
}

impl DependencyStatus {
    pub fn check() -> Self {
        Self {
            tesseract: Command::new("tesseract")
                .arg("--version")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false),
            pdftoppm: Command::new("pdftoppm")
                .arg("-v")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false),
            magick: Command::new("magick")
                .arg("--version")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false),
            tabula: Command::new("tabula")
                .arg("--version")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false),
        }
    }

    pub fn can_ocr(&self) -> bool {
        self.tesseract && (self.pdftoppm || self.magick)
    }
}

pub fn get_dependency_status() -> DependencyStatus {
    DEPS_CHECKED.get_or_init(DependencyStatus::check).clone()
}

pub async fn extract(request: &PdfRequest) -> Result<PdfResponse> {
    let path = Path::new(&request.path);

    if !path.exists() {
        anyhow::bail!("PDF file not found: {}", request.path);
    }

    let bytes = std::fs::read(path)?;

    let mut text =
        pdf_extract::extract_text_from_mem(&bytes).context("Failed to extract text from PDF")?;

    let doc = lopdf::Document::load(path).context("Failed to load PDF document")?;

    let pages = doc.get_pages().len();

    if request.ocr && is_text_sparse(&text, pages) {
        if let Ok(ocr_text) = perform_ocr(path).await {
            text = ocr_text;
        }
    }

    let metadata = extract_metadata(&doc);

    let tables = if request.extract_tables {
        let deps = get_dependency_status();
        if deps.tabula {
            match extract_tables_with_tabula(path).await {
                Ok(t) if !t.is_empty() => Some(t),
                _ => Some(extract_tables_heuristic(&text)),
            }
        } else {
            Some(extract_tables_heuristic(&text))
        }
    } else {
        None
    };

    Ok(PdfResponse {
        text,
        pages,
        tables,
        metadata,
    })
}

fn is_text_sparse(text: &str, pages: usize) -> bool {
    let chars_per_page = text.len() / pages.max(1);
    chars_per_page < 100
}

async fn perform_ocr(pdf_path: &Path) -> Result<String> {
    let deps = get_dependency_status();

    if !deps.can_ocr() {
        let mut missing = Vec::new();
        if !deps.tesseract {
            missing.push("tesseract");
        }
        if !deps.pdftoppm && !deps.magick {
            missing.push("pdftoppm (poppler) or ImageMagick");
        }
        anyhow::bail!(
            "OCR requires: {}. Install with:\n  macOS: brew install tesseract tesseract-lang poppler\n  Linux: sudo apt install tesseract-ocr poppler-utils",
            missing.join(", ")
        );
    }

    let temp_dir = tempfile::tempdir()?;
    let temp_path = temp_dir.path();

    if deps.pdftoppm {
        let pdftoppm_output = Command::new("pdftoppm")
            .args([
                "-png",
                "-r",
                "300",
                pdf_path.to_str().unwrap(),
                temp_path.join("page").to_str().unwrap(),
            ])
            .output();

        if let Ok(result) = pdftoppm_output {
            if result.status.success() {
                return ocr_images_in_dir(temp_path).await;
            }
        }
    }

    if deps.magick {
        return ocr_with_magick(pdf_path, temp_path).await;
    }

    anyhow::bail!("No PDF to image converter available")
}

async fn ocr_images_in_dir(temp_path: &Path) -> Result<String> {
    let mut all_text = String::new();
    let mut page_files: Vec<_> = std::fs::read_dir(temp_path)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "png"))
        .collect();

    page_files.sort_by_key(|e| e.path());

    for entry in page_files {
        let img_path = entry.path();
        if let Ok(page_text) = run_tesseract(&img_path) {
            all_text.push_str(&page_text);
            all_text.push('\n');
        }
    }

    if all_text.trim().is_empty() {
        anyhow::bail!("OCR produced no text");
    }

    Ok(all_text)
}

async fn ocr_with_magick(pdf_path: &Path, temp_path: &Path) -> Result<String> {
    let convert_result = Command::new("magick")
        .args([
            "convert",
            "-density",
            "300",
            pdf_path.to_str().unwrap(),
            "-depth",
            "8",
            temp_path.join("page.png").to_str().unwrap(),
        ])
        .output()?;

    if !convert_result.status.success() {
        let stderr = String::from_utf8_lossy(&convert_result.stderr);
        anyhow::bail!("ImageMagick convert failed: {}", stderr);
    }

    ocr_images_in_dir(temp_path).await
}

fn run_tesseract(image_path: &Path) -> Result<String> {
    let deps = get_dependency_status();
    if !deps.tesseract {
        anyhow::bail!(get_tesseract_install_message());
    }

    let output = Command::new("tesseract")
        .args([
            image_path.to_str().unwrap(),
            "stdout",
            "-l",
            "eng+kor+jpn+chi_sim",
        ])
        .output();

    match output {
        Ok(result) if result.status.success() => {
            Ok(String::from_utf8_lossy(&result.stdout).to_string())
        }
        Ok(result) => {
            let output_simple = Command::new("tesseract")
                .args([image_path.to_str().unwrap(), "stdout"])
                .output()?;

            if output_simple.status.success() {
                Ok(String::from_utf8_lossy(&output_simple.stdout).to_string())
            } else {
                let stderr = String::from_utf8_lossy(&result.stderr);
                anyhow::bail!("Tesseract failed: {}", stderr)
            }
        }
        Err(e) => anyhow::bail!("Failed to run tesseract: {}", e),
    }
}

fn get_tesseract_install_message() -> String {
    let platform_cmd = if cfg!(target_os = "macos") {
        "brew install tesseract tesseract-lang"
    } else if cfg!(target_os = "windows") {
        "choco install tesseract\n  Or download from: https://github.com/UB-Mannheim/tesseract/wiki"
    } else {
        "sudo apt install tesseract-ocr tesseract-ocr-kor tesseract-ocr-jpn tesseract-ocr-chi-sim"
    };

    format!(
        "Tesseract OCR not installed. OCR is required for scanned PDFs.\n\nInstall with:\n  {}",
        platform_cmd
    )
}

fn extract_metadata(doc: &lopdf::Document) -> PdfMetadata {
    let info = doc
        .trailer
        .get(b"Info")
        .ok()
        .and_then(|obj| obj.as_reference().ok())
        .and_then(|reference| doc.get_object(reference).ok());

    let mut title = None;
    let mut author = None;
    let mut subject = None;
    let mut creator = None;

    if let Some(lopdf::Object::Dictionary(dict)) = info {
        title = dict.get(b"Title").ok().and_then(extract_string);
        author = dict.get(b"Author").ok().and_then(extract_string);
        subject = dict.get(b"Subject").ok().and_then(extract_string);
        creator = dict.get(b"Creator").ok().and_then(extract_string);
    }

    PdfMetadata {
        title,
        author,
        subject,
        creator,
    }
}

fn extract_string(obj: &lopdf::Object) -> Option<String> {
    match obj {
        lopdf::Object::String(bytes, _) => String::from_utf8(bytes.clone()).ok(),
        _ => None,
    }
}

async fn extract_tables_with_tabula(pdf_path: &Path) -> Result<Vec<String>> {
    let output = Command::new("tabula")
        .args(["-f", "tsv", "-p", "all", pdf_path.to_str().unwrap()])
        .output()
        .context("Failed to run tabula")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Tabula failed: {}", stderr);
    }

    let tsv_output = String::from_utf8_lossy(&output.stdout);
    let tables = parse_tabula_output(&tsv_output);

    Ok(tables)
}

fn parse_tabula_output(tsv: &str) -> Vec<String> {
    let mut tables = Vec::new();
    let mut current_table: Vec<Vec<String>> = Vec::new();

    for line in tsv.lines() {
        if line.trim().is_empty() {
            if current_table.len() >= 2 {
                tables.push(table_to_markdown(&current_table));
            }
            current_table.clear();
            continue;
        }

        let cells: Vec<String> = line.split('\t').map(|s| s.trim().to_string()).collect();

        if cells.iter().any(|c| !c.is_empty()) {
            current_table.push(cells);
        }
    }

    if current_table.len() >= 2 {
        tables.push(table_to_markdown(&current_table));
    }

    tables
}

fn table_to_markdown(rows: &[Vec<String>]) -> String {
    if rows.is_empty() {
        return String::new();
    }

    let col_count = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let mut result = Vec::new();

    for (idx, row) in rows.iter().enumerate() {
        let mut cells: Vec<String> = row.clone();
        while cells.len() < col_count {
            cells.push(String::new());
        }

        let line = format!("| {} |", cells.join(" | "));
        result.push(line);

        if idx == 0 {
            let separator = format!(
                "| {} |",
                (0..col_count)
                    .map(|_| "---")
                    .collect::<Vec<_>>()
                    .join(" | ")
            );
            result.push(separator);
        }
    }

    result.join("\n")
}

fn extract_tables_heuristic(text: &str) -> Vec<String> {
    let mut tables = Vec::new();
    let lines: Vec<&str> = text.lines().collect();

    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];

        let tab_count = line.matches('\t').count();
        let pipe_count = line.matches('|').count();

        if tab_count >= 2 || pipe_count >= 2 {
            let mut table_lines = vec![line.to_string()];
            let delimiter_count = std::cmp::max(tab_count, pipe_count);

            i += 1;
            while i < lines.len() {
                let next_line = lines[i];
                let next_tab = next_line.matches('\t').count();
                let next_pipe = next_line.matches('|').count();

                if next_tab >= delimiter_count - 1 || next_pipe >= delimiter_count - 1 {
                    table_lines.push(next_line.to_string());
                    i += 1;
                } else {
                    break;
                }
            }

            if table_lines.len() >= 2 {
                let markdown_table = convert_to_markdown_table(&table_lines);
                tables.push(markdown_table);
            }
        } else {
            i += 1;
        }
    }

    tables
}

fn convert_to_markdown_table(lines: &[String]) -> String {
    let mut result = Vec::new();

    for (idx, line) in lines.iter().enumerate() {
        let cells: Vec<&str> = if line.contains('|') {
            line.split('|').map(|s| s.trim()).collect()
        } else {
            line.split('\t').map(|s| s.trim()).collect()
        };

        let row = format!("| {} |", cells.join(" | "));
        result.push(row);

        if idx == 0 {
            let separator = format!(
                "| {} |",
                cells.iter().map(|_| "---").collect::<Vec<_>>().join(" | ")
            );
            result.push(separator);
        }
    }

    result.join("\n")
}

/// Extract only tables from a PDF file
pub async fn extract_tables_only(path: &str) -> Result<Vec<String>> {
    let pdf_path = Path::new(path);

    if !pdf_path.exists() {
        anyhow::bail!("PDF file not found: {}", path);
    }

    let deps = get_dependency_status();

    // Try tabula first (best quality)
    if deps.tabula {
        if let Ok(tables) = extract_tables_with_tabula(pdf_path).await {
            if !tables.is_empty() {
                return Ok(tables);
            }
        }
    }

    // Fallback to text-based heuristic
    let bytes = std::fs::read(pdf_path)?;
    let text =
        pdf_extract::extract_text_from_mem(&bytes).context("Failed to extract text from PDF")?;

    Ok(extract_tables_heuristic(&text))
}
