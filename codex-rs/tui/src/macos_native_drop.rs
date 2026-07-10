use serde::Deserialize;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NativeMedicalFileDropEvent {
    pub(crate) schema_version: u32,
    pub(crate) session_id: String,
    pub(crate) target_pane: String,
    pub(crate) paths: Vec<PathBuf>,
    pub(crate) screen_x: Option<f64>,
    pub(crate) screen_y: Option<f64>,
    pub(crate) modifiers: Vec<String>,
    pub(crate) source: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum NativeMedicalDropResult {
    Dropped(NativeMedicalFileDropEvent),
    Canceled,
    Unsupported(String),
    Failed(String),
}

#[derive(Debug, Deserialize)]
struct NativeMedicalFileDropWire {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(default)]
    schema_version: u32,
    #[serde(default)]
    session_id: String,
    #[serde(default)]
    target_pane: String,
    #[serde(default)]
    paths: Vec<String>,
    #[serde(default)]
    screen_x: Option<f64>,
    #[serde(default)]
    screen_y: Option<f64>,
    #[serde(default)]
    modifiers: Vec<String>,
    #[serde(default)]
    source: String,
}

pub(crate) fn parse_native_medical_file_drop_event(
    json: &str,
) -> Result<NativeMedicalFileDropEvent, String> {
    let wire: NativeMedicalFileDropWire = serde_json::from_str(json)
        .map_err(|err| format!("Native file drop event was not valid JSON: {err}"))?;
    if wire.event_type == "medical_file_drop_cancelled" {
        return Err("cancelled".to_string());
    }
    if wire.event_type != "medical_file_drop" {
        return Err(format!(
            "Native file drop event had unexpected type `{}`.",
            wire.event_type
        ));
    }
    if wire.schema_version != 1 {
        return Err(format!(
            "Native file drop schema version {} is unsupported.",
            wire.schema_version
        ));
    }
    if wire.paths.is_empty() {
        return Err("Native file drop did not include any file paths.".to_string());
    }
    Ok(NativeMedicalFileDropEvent {
        schema_version: wire.schema_version,
        session_id: wire.session_id,
        target_pane: wire.target_pane,
        paths: wire.paths.into_iter().map(PathBuf::from).collect(),
        screen_x: wire.screen_x,
        screen_y: wire.screen_y,
        modifiers: wire.modifiers,
        source: if wire.source.trim().is_empty() {
            "macos_native_drop".to_string()
        } else {
            wire.source
        },
    })
}

pub(crate) fn run_native_medical_file_drop_panel() -> NativeMedicalDropResult {
    run_native_medical_file_drop_panel_impl()
}

#[cfg(target_os = "macos")]
fn run_native_medical_file_drop_panel_impl() -> NativeMedicalDropResult {
    let Some((program, args)) = swift_command() else {
        return NativeMedicalDropResult::Unsupported(
            "macOS native drop requires Swift; paste a local JPG/PDF path instead.".to_string(),
        );
    };
    let Ok(temp_dir) = tempfile::tempdir() else {
        return NativeMedicalDropResult::Failed(
            "Could not prepare the macOS native drop panel.".to_string(),
        );
    };
    let script_path = temp_dir.path().join("codex_medical_file_drop.swift");
    if let Err(err) = fs::write(&script_path, MACOS_MEDICAL_FILE_DROP_SWIFT) {
        return NativeMedicalDropResult::Failed(format!(
            "Could not write the macOS native drop panel: {err}"
        ));
    }

    let output = Command::new(program).args(args).arg(&script_path).output();
    match output {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let Some(json_line) = stdout.lines().rev().find(|line| !line.trim().is_empty()) else {
                return NativeMedicalDropResult::Canceled;
            };
            if json_line.contains("\"medical_file_drop_cancelled\"") {
                return NativeMedicalDropResult::Canceled;
            }
            match parse_native_medical_file_drop_event(json_line) {
                Ok(event) => NativeMedicalDropResult::Dropped(event),
                Err(err) => NativeMedicalDropResult::Failed(err),
            }
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            NativeMedicalDropResult::Failed(format!(
                "macOS native drop panel failed: {}",
                compact_error(stderr.trim())
            ))
        }
        Err(err) => {
            NativeMedicalDropResult::Failed(format!("macOS native drop panel failed: {err}"))
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn run_native_medical_file_drop_panel_impl() -> NativeMedicalDropResult {
    NativeMedicalDropResult::Unsupported(
        "Native file drop is macOS-only; paste a local JPG/PDF path instead.".to_string(),
    )
}

#[cfg(target_os = "macos")]
fn swift_command() -> Option<(&'static str, Vec<&'static str>)> {
    if Path::new("/usr/bin/swift").is_file() {
        return Some(("/usr/bin/swift", Vec::new()));
    }
    if Path::new("/usr/bin/xcrun").is_file() {
        return Some(("/usr/bin/xcrun", vec!["swift"]));
    }
    None
}

fn compact_error(message: &str) -> String {
    let trimmed = message.trim();
    if trimmed.is_empty() {
        return "no error output".to_string();
    }
    let mut out = String::new();
    for ch in trimmed.chars().take(180) {
        out.push(ch);
    }
    if trimmed.chars().count() > 180 {
        out.push_str("...");
    }
    out
}

#[cfg(target_os = "macos")]
const MACOS_MEDICAL_FILE_DROP_SWIFT: &str = r#"
import AppKit
import Foundation
import QuickLookThumbnailing

func emitJSON(_ object: [String: Any]) {
    if let data = try? JSONSerialization.data(withJSONObject: object, options: []),
       let text = String(data: data, encoding: .utf8) {
        FileHandle.standardOutput.write((text + "\n").data(using: .utf8)!)
        fflush(stdout)
    }
}

func isPriorityFile(_ url: URL) -> Bool {
    let ext = url.pathExtension.lowercased()
    return ext == "jpg" || ext == "jpeg" || ext == "pdf"
}

func fileTypeLabel(_ url: URL) -> String {
    let ext = url.pathExtension.lowercased()
    if ext == "pdf" { return "PDF" }
    if ext == "jpg" || ext == "jpeg" { return "JPG" }
    return ext.uppercased()
}

final class DropRow: NSStackView {
    private let imageView = NSImageView()
    private let title = NSTextField(labelWithString: "")
    private let subtitle = NSTextField(labelWithString: "")

    init(url: URL) {
        super.init(frame: .zero)
        orientation = .horizontal
        alignment = .centerY
        spacing = 10

        imageView.imageScaling = .scaleProportionallyUpOrDown
        imageView.setFrameSize(NSSize(width: 56, height: 56))
        imageView.image = NSWorkspace.shared.icon(forFile: url.path)
        addArrangedSubview(imageView)

        let labels = NSStackView()
        labels.orientation = .vertical
        labels.spacing = 2
        title.stringValue = url.lastPathComponent
        title.font = NSFont.systemFont(ofSize: 13, weight: .semibold)
        title.lineBreakMode = .byTruncatingMiddle
        subtitle.stringValue = "\(fileTypeLabel(url)) · metadata-only local reference"
        subtitle.textColor = .secondaryLabelColor
        subtitle.font = NSFont.systemFont(ofSize: 11)
        labels.addArrangedSubview(title)
        labels.addArrangedSubview(subtitle)
        addArrangedSubview(labels)

        let request = QLThumbnailGenerator.Request(
            fileAt: url,
            size: CGSize(width: 112, height: 112),
            scale: NSScreen.main?.backingScaleFactor ?? 2.0,
            representationTypes: .thumbnail
        )
        QLThumbnailGenerator.shared.generateBestRepresentation(for: request) { [weak self] representation, _ in
            DispatchQueue.main.async {
                if let image = representation?.nsImage {
                    self?.imageView.image = image
                }
            }
        }
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
}

final class MedicalDropView: NSView {
    var urls: [URL] = [] {
        didSet { rebuildList() }
    }
    var lastDropScreenX: Double?
    var lastDropScreenY: Double?

    private let title = NSTextField(labelWithString: "Drop JPG/PDF patient files")
    private let subtitle = NSTextField(labelWithString: "Saved as local references only. Originals stay where they are.")
    private let status = NSTextField(labelWithString: "Drop scanned ID, insurance card, or referral PDF here.")
    private let list = NSStackView()

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        layer?.cornerRadius = 10
        layer?.borderWidth = 2
        layer?.borderColor = NSColor.separatorColor.cgColor
        registerForDraggedTypes([.fileURL])

        let root = NSStackView()
        root.orientation = .vertical
        root.spacing = 12
        root.translatesAutoresizingMaskIntoConstraints = false
        addSubview(root)

        title.font = NSFont.systemFont(ofSize: 20, weight: .bold)
        subtitle.textColor = .secondaryLabelColor
        status.textColor = .secondaryLabelColor
        status.font = NSFont.systemFont(ofSize: 12)

        list.orientation = .vertical
        list.spacing = 8

        root.addArrangedSubview(title)
        root.addArrangedSubview(subtitle)
        root.addArrangedSubview(status)
        root.addArrangedSubview(list)

        NSLayoutConstraint.activate([
            root.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 20),
            root.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -20),
            root.topAnchor.constraint(equalTo: topAnchor, constant: 18),
            root.bottomAnchor.constraint(lessThanOrEqualTo: bottomAnchor, constant: -18)
        ])
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func draggingEntered(_ sender: NSDraggingInfo) -> NSDragOperation {
        return validURLs(sender).isEmpty ? [] : .copy
    }

    override func prepareForDragOperation(_ sender: NSDraggingInfo) -> Bool {
        return !validURLs(sender).isEmpty
    }

    override func performDragOperation(_ sender: NSDraggingInfo) -> Bool {
        let dropped = validURLs(sender)
        if dropped.isEmpty {
            status.stringValue = "Only JPG/JPEG/PDF files can be added in this pass."
            return false
        }
        let localPoint = convert(sender.draggingLocation, from: nil)
        if let window = window {
            let screenRect = window.convertToScreen(NSRect(origin: localPoint, size: .zero))
            lastDropScreenX = Double(screenRect.origin.x)
            lastDropScreenY = Double(screenRect.origin.y)
        }
        appendURLs(dropped)
        return true
    }

    func appendURLs(_ incoming: [URL]) {
        var seen = Set(urls.map { $0.standardizedFileURL.path })
        for url in incoming where isPriorityFile(url) && seen.insert(url.standardizedFileURL.path).inserted {
            urls.append(url.standardizedFileURL)
        }
        status.stringValue = urls.isEmpty
            ? "Drop scanned ID, insurance card, or referral PDF here."
            : "\(urls.count) file reference(s) ready. Nothing is uploaded, copied, OCR'd, or sent to agent."
    }

    private func validURLs(_ sender: NSDraggingInfo) -> [URL] {
        let options: [NSPasteboard.ReadingOptionKey: Any] = [.urlReadingFileURLsOnly: true]
        let objects = sender.draggingPasteboard.readObjects(forClasses: [NSURL.self], options: options) as? [NSURL] ?? []
        return objects.map { $0 as URL }.filter { isPriorityFile($0) }
    }

    private func rebuildList() {
        list.arrangedSubviews.forEach { view in
            list.removeArrangedSubview(view)
            view.removeFromSuperview()
        }
        for url in urls.prefix(8) {
            list.addArrangedSubview(DropRow(url: url))
        }
        if urls.count > 8 {
            list.addArrangedSubview(NSTextField(labelWithString: "...and \(urls.count - 8) more"))
        }
    }
}

final class Controller: NSObject, NSWindowDelegate {
    let app = NSApplication.shared
    let dropView = MedicalDropView(frame: .zero)
    var window: NSWindow!
    var didFinish = false

    func show() {
        let choose = NSButton(title: "Choose Files", target: self, action: #selector(chooseFiles))
        let use = NSButton(title: "Use Files", target: self, action: #selector(useFiles))
        use.keyEquivalent = "\r"
        let cancel = NSButton(title: "Cancel", target: self, action: #selector(cancel))

        let buttons = NSStackView(views: [choose, cancel, use])
        buttons.orientation = .horizontal
        buttons.alignment = .centerY
        buttons.spacing = 8
        buttons.translatesAutoresizingMaskIntoConstraints = false

        let content = NSView()
        dropView.translatesAutoresizingMaskIntoConstraints = false
        content.addSubview(dropView)
        content.addSubview(buttons)

        NSLayoutConstraint.activate([
            dropView.leadingAnchor.constraint(equalTo: content.leadingAnchor, constant: 16),
            dropView.trailingAnchor.constraint(equalTo: content.trailingAnchor, constant: -16),
            dropView.topAnchor.constraint(equalTo: content.topAnchor, constant: 16),
            dropView.bottomAnchor.constraint(equalTo: buttons.topAnchor, constant: -12),
            buttons.trailingAnchor.constraint(equalTo: content.trailingAnchor, constant: -16),
            buttons.bottomAnchor.constraint(equalTo: content.bottomAnchor, constant: -16)
        ])

        window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 620, height: 430),
            styleMask: [.titled, .closable, .miniaturizable],
            backing: .buffered,
            defer: false
        )
        window.title = "Patient File Drop"
        window.contentView = content
        window.center()
        window.delegate = self
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }

    @objc func chooseFiles() {
        let panel = NSOpenPanel()
        panel.message = "Choose JPG/PDF patient files"
        panel.allowsMultipleSelection = true
        panel.canChooseDirectories = false
        panel.canChooseFiles = true
        panel.allowedFileTypes = ["jpg", "jpeg", "pdf"]
        if panel.runModal() == .OK {
            dropView.appendURLs(panel.urls)
        }
    }

    @objc func useFiles() {
        if dropView.urls.isEmpty {
            dropView.appendURLs([])
            return
        }
        didFinish = true
        var event: [String: Any] = [
            "type": "medical_file_drop",
            "schema_version": 1,
            "session_id": ProcessInfo.processInfo.environment["CODEX_MEDICAL_FILE_DROP_SESSION"] ?? "",
            "target_pane": "patient_file_tree",
            "paths": dropView.urls.map { $0.path },
            "modifiers": [],
            "source": "macos_native_drop"
        ]
        if let x = dropView.lastDropScreenX { event["screen_x"] = x }
        if let y = dropView.lastDropScreenY { event["screen_y"] = y }
        emitJSON(event)
        app.terminate(nil)
    }

    @objc func cancel() {
        if didFinish { return }
        didFinish = true
        emitJSON(["type": "medical_file_drop_cancelled"])
        app.terminate(nil)
    }

    func windowWillClose(_ notification: Notification) {
        cancel()
    }
}

let app = NSApplication.shared
app.setActivationPolicy(.regular)
let controller = Controller()
controller.show()
app.run()
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_native_medical_file_drop_event() {
        let json = r#"{
            "type":"medical_file_drop",
            "schema_version":1,
            "session_id":"session-1",
            "target_pane":"patient_file_tree",
            "paths":["/tmp/id.jpg","/tmp/referral.pdf"],
            "screen_x":412.0,
            "screen_y":903.0,
            "modifiers":["option"],
            "source":"macos_native_drop"
        }"#;

        let event = parse_native_medical_file_drop_event(json).expect("parse event");
        assert_eq!(event.schema_version, 1);
        assert_eq!(event.session_id, "session-1");
        assert_eq!(event.target_pane, "patient_file_tree");
        assert_eq!(
            event.paths,
            vec![
                PathBuf::from("/tmp/id.jpg"),
                PathBuf::from("/tmp/referral.pdf")
            ]
        );
        assert_eq!(event.screen_x, Some(412.0));
        assert_eq!(event.screen_y, Some(903.0));
        assert_eq!(event.modifiers, vec!["option"]);
        assert_eq!(event.source, "macos_native_drop");
    }

    #[test]
    fn rejects_wrong_native_event_type() {
        let err = parse_native_medical_file_drop_event(
            r#"{"type":"other","schema_version":1,"paths":["/tmp/id.jpg"]}"#,
        )
        .expect_err("wrong type should fail");
        assert!(err.contains("unexpected type"));
    }

    #[test]
    fn compact_error_bounds_long_output() {
        let compact = compact_error(&"x".repeat(240));
        assert!(compact.len() <= 183);
        assert!(compact.ends_with("..."));
    }
}
