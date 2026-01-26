use serde::{Deserialize, Serialize};
use std::process::Command;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TtsError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TTS not available: {0}")]
    NotAvailable(String),
    #[error("TTS failed: {0}")]
    Failed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsRequest {
    pub text: String,
    pub voice: Option<String>,
    pub rate: Option<f32>,
    pub output_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsVoice {
    pub id: String,
    pub name: String,
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsStatus {
    pub available: bool,
    pub engine: String,
    pub voices: Vec<TtsVoice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TtsEngine {
    System,
    Say,
    Espeak,
}

impl Default for TtsEngine {
    fn default() -> Self {
        if cfg!(target_os = "macos") {
            Self::Say
        } else {
            Self::Espeak
        }
    }
}

pub fn check_tts_status() -> TtsStatus {
    if cfg!(target_os = "macos") {
        check_macos_tts()
    } else if cfg!(target_os = "linux") {
        check_linux_tts()
    } else if cfg!(target_os = "windows") {
        check_windows_tts()
    } else {
        TtsStatus {
            available: false,
            engine: "none".to_string(),
            voices: Vec::new(),
        }
    }
}

fn check_macos_tts() -> TtsStatus {
    let output = Command::new("say").args(["-v", "?"]).output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let voices: Vec<TtsVoice> = stdout
                .lines()
                .filter_map(|line| {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        Some(TtsVoice {
                            id: parts[0].to_string(),
                            name: parts[0].to_string(),
                            language: parts[1].to_string(),
                        })
                    } else {
                        None
                    }
                })
                .collect();

            TtsStatus {
                available: true,
                engine: "say".to_string(),
                voices,
            }
        }
        _ => TtsStatus {
            available: false,
            engine: "say".to_string(),
            voices: Vec::new(),
        },
    }
}

fn check_linux_tts() -> TtsStatus {
    let output = Command::new("espeak").arg("--voices").output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let voices: Vec<TtsVoice> = stdout
                .lines()
                .skip(1)
                .filter_map(|line| {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 4 {
                        Some(TtsVoice {
                            id: parts[4].to_string(),
                            name: parts[3].to_string(),
                            language: parts[1].to_string(),
                        })
                    } else {
                        None
                    }
                })
                .collect();

            TtsStatus {
                available: true,
                engine: "espeak".to_string(),
                voices,
            }
        }
        _ => {
            let piper = Command::new("piper").arg("--help").output();

            if piper.is_ok() {
                TtsStatus {
                    available: true,
                    engine: "piper".to_string(),
                    voices: Vec::new(),
                }
            } else {
                TtsStatus {
                    available: false,
                    engine: "none".to_string(),
                    voices: Vec::new(),
                }
            }
        }
    }
}

fn check_windows_tts() -> TtsStatus {
    TtsStatus {
        available: true,
        engine: "sapi".to_string(),
        voices: Vec::new(),
    }
}

pub fn speak(req: TtsRequest) -> Result<(), TtsError> {
    let status = check_tts_status();
    if !status.available {
        return Err(TtsError::NotAvailable(
            "No TTS engine available".to_string(),
        ));
    }

    match status.engine.as_str() {
        "say" => speak_macos(req),
        "espeak" => speak_espeak(req),
        "piper" => speak_piper(req),
        "sapi" => speak_windows(req),
        _ => Err(TtsError::NotAvailable(format!(
            "Unknown engine: {}",
            status.engine
        ))),
    }
}

fn speak_macos(req: TtsRequest) -> Result<(), TtsError> {
    let mut cmd = Command::new("say");

    if let Some(voice) = req.voice {
        cmd.args(["-v", &voice]);
    }

    if let Some(rate) = req.rate {
        let words_per_minute = (rate * 175.0) as i32;
        cmd.args(["-r", &words_per_minute.to_string()]);
    }

    if let Some(output_path) = req.output_path {
        cmd.args(["-o", &output_path]);
    }

    cmd.arg(&req.text);

    let output = cmd.output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(TtsError::Failed(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ))
    }
}

fn speak_espeak(req: TtsRequest) -> Result<(), TtsError> {
    let mut cmd = Command::new("espeak");

    if let Some(voice) = req.voice {
        cmd.args(["-v", &voice]);
    }

    if let Some(rate) = req.rate {
        let speed = (rate * 175.0) as i32;
        cmd.args(["-s", &speed.to_string()]);
    }

    if let Some(output_path) = req.output_path {
        cmd.args(["-w", &output_path]);
    }

    cmd.arg(&req.text);

    let output = cmd.output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(TtsError::Failed(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ))
    }
}

fn speak_piper(req: TtsRequest) -> Result<(), TtsError> {
    let mut cmd = Command::new("piper");

    cmd.args([
        "--output_file",
        req.output_path.as_deref().unwrap_or("/dev/stdout"),
    ]);

    if let Some(voice) = req.voice {
        cmd.args(["--model", &voice]);
    }

    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());

    let mut child = cmd.spawn()?;

    if let Some(stdin) = child.stdin.as_mut() {
        use std::io::Write;
        stdin.write_all(req.text.as_bytes())?;
    }

    let output = child.wait_with_output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(TtsError::Failed(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ))
    }
}

fn speak_windows(req: TtsRequest) -> Result<(), TtsError> {
    let script = format!(
        r#"Add-Type -AssemblyName System.Speech; $synth = New-Object System.Speech.Synthesis.SpeechSynthesizer; {} $synth.Speak('{}')"#,
        if let Some(ref voice) = req.voice {
            format!("$synth.SelectVoice('{}');", voice)
        } else {
            String::new()
        },
        req.text.replace("'", "''")
    );

    let output = Command::new("powershell")
        .args(["-Command", &script])
        .output()?;

    if output.status.success() {
        Ok(())
    } else {
        Err(TtsError::Failed(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ))
    }
}

pub fn speak_to_file(req: TtsRequest, output_path: &str) -> Result<String, TtsError> {
    let req_with_output = TtsRequest {
        output_path: Some(output_path.to_string()),
        ..req
    };
    speak(req_with_output)?;
    Ok(output_path.to_string())
}

pub fn list_voices() -> Result<Vec<TtsVoice>, TtsError> {
    let status = check_tts_status();
    if !status.available {
        return Err(TtsError::NotAvailable(
            "No TTS engine available".to_string(),
        ));
    }
    Ok(status.voices)
}

pub fn stop_speaking() -> Result<(), TtsError> {
    if cfg!(target_os = "macos") {
        Command::new("killall").arg("say").output().ok();
    } else if cfg!(target_os = "linux") {
        Command::new("killall").arg("espeak").output().ok();
        Command::new("killall").arg("piper").output().ok();
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadArticleRequest {
    pub content: String,
    pub voice: Option<String>,
    pub rate: Option<f32>,
}

pub fn read_article(req: ReadArticleRequest) -> Result<(), TtsError> {
    let chunks = split_into_sentences(&req.content);

    for chunk in chunks {
        speak(TtsRequest {
            text: chunk,
            voice: req.voice.clone(),
            rate: req.rate,
            output_path: None,
        })?;
    }

    Ok(())
}

fn split_into_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();

    for c in text.chars() {
        current.push(c);
        if c == '.' || c == '!' || c == '?' || c == '\n' {
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() {
                sentences.push(trimmed);
            }
            current = String::new();
        }
    }

    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        sentences.push(trimmed);
    }

    sentences
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_into_sentences_period() {
        let sentences = split_into_sentences("Hello world. How are you. I am fine.");
        assert_eq!(sentences.len(), 3);
        assert_eq!(sentences[0], "Hello world.");
        assert_eq!(sentences[1], "How are you.");
        assert_eq!(sentences[2], "I am fine.");
    }

    #[test]
    fn test_split_into_sentences_question() {
        let sentences = split_into_sentences("What is this? Why is it here?");
        assert_eq!(sentences.len(), 2);
        assert_eq!(sentences[0], "What is this?");
        assert_eq!(sentences[1], "Why is it here?");
    }

    #[test]
    fn test_split_into_sentences_exclamation() {
        let sentences = split_into_sentences("Wow! Amazing! Incredible!");
        assert_eq!(sentences.len(), 3);
    }

    #[test]
    fn test_split_into_sentences_newline() {
        let sentences = split_into_sentences("First line\nSecond line\nThird line");
        assert_eq!(sentences.len(), 3);
    }

    #[test]
    fn test_split_into_sentences_mixed() {
        let sentences = split_into_sentences("Hello. How are you? Great!\nNew paragraph.");
        assert_eq!(sentences.len(), 4);
    }

    #[test]
    fn test_split_into_sentences_no_terminator() {
        let sentences = split_into_sentences("No ending punctuation");
        assert_eq!(sentences.len(), 1);
        assert_eq!(sentences[0], "No ending punctuation");
    }

    #[test]
    fn test_split_into_sentences_empty() {
        let sentences = split_into_sentences("");
        assert!(sentences.is_empty());
    }

    #[test]
    fn test_split_into_sentences_whitespace_only() {
        let sentences = split_into_sentences("   \n\n   ");
        assert!(sentences.is_empty());
    }

    #[test]
    fn test_split_into_sentences_single_sentence() {
        let sentences = split_into_sentences("Just one sentence.");
        assert_eq!(sentences.len(), 1);
        assert_eq!(sentences[0], "Just one sentence.");
    }

    #[test]
    fn test_tts_engine_default() {
        let engine = TtsEngine::default();
        if cfg!(target_os = "macos") {
            assert!(matches!(engine, TtsEngine::Say));
        } else {
            assert!(matches!(engine, TtsEngine::Espeak));
        }
    }

    #[test]
    fn test_tts_request_serialization() {
        let request = TtsRequest {
            text: "Hello world".to_string(),
            voice: Some("Alex".to_string()),
            rate: Some(1.0),
            output_path: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: TtsRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.text, "Hello world");
        assert_eq!(deserialized.voice, Some("Alex".to_string()));
        assert_eq!(deserialized.rate, Some(1.0));
    }

    #[test]
    fn test_tts_voice_serialization() {
        let voice = TtsVoice {
            id: "alex".to_string(),
            name: "Alex".to_string(),
            language: "en_US".to_string(),
        };

        let json = serde_json::to_string(&voice).unwrap();
        let deserialized: TtsVoice = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, "alex");
        assert_eq!(deserialized.name, "Alex");
    }

    #[test]
    fn test_tts_status_serialization() {
        let status = TtsStatus {
            available: true,
            engine: "say".to_string(),
            voices: vec![TtsVoice {
                id: "alex".to_string(),
                name: "Alex".to_string(),
                language: "en_US".to_string(),
            }],
        };

        let json = serde_json::to_string(&status).unwrap();
        let deserialized: TtsStatus = serde_json::from_str(&json).unwrap();

        assert!(deserialized.available);
        assert_eq!(deserialized.engine, "say");
        assert_eq!(deserialized.voices.len(), 1);
    }

    #[test]
    fn test_read_article_request_serialization() {
        let request = ReadArticleRequest {
            content: "Article content here.".to_string(),
            voice: Some("Alex".to_string()),
            rate: Some(1.5),
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: ReadArticleRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.content, "Article content here.");
        assert_eq!(deserialized.voice, Some("Alex".to_string()));
        assert_eq!(deserialized.rate, Some(1.5));
    }

    #[test]
    fn test_check_tts_status() {
        let status = check_tts_status();
        assert!(!status.engine.is_empty());
    }

    #[test]
    fn test_tts_error_display() {
        let io_err = TtsError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        ));
        assert!(io_err.to_string().contains("IO error"));

        let not_available = TtsError::NotAvailable("No engine".to_string());
        assert!(not_available.to_string().contains("not available"));

        let failed = TtsError::Failed("Command failed".to_string());
        assert!(failed.to_string().contains("failed"));
    }
}
