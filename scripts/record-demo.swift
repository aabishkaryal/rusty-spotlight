// Records the README's demo: assets/demo.mp4 and its poster, assets/demo-poster.png.
//
//   swift scripts/record-demo.swift                 (from the repository root)
//   swift scripts/record-demo.swift --encode-only   re-encode the last recording
//
// Rusty Spotlight must be installed and running on Command-Space. The script
// covers the panel's area with a backdrop of its own (no wallpaper or other
// windows end up in the recording), records that area with `screencapture`,
// plays a timed sequence of key presses, and saves an H.264 MP4 (plays in
// every browser) at the screen's full resolution, plus a still for the README.
// The app running it needs permission to post key events (Accessibility).
// Your screen shows the demo for about 35 seconds; don't type meanwhile.

import AppKit
import AVFoundation
import ImageIO
import UniformTypeIdentifiers

let seconds = 34.0
let out = "assets/demo.mp4"
let poster = "assets/demo-poster.png"
/// Screen recordings of flat colour compress well; 2.4 Mbit/s keeps text
/// sharp and the file under the 10 MB GitHub accepts for README videos.
let bitrate = 2_400_000
/// When the poster is taken, in seconds from the start of recording.
var posterAt = 14.0
var recordingStarted = Date()
let movie = NSTemporaryDirectory() + "rusty-spotlight-demo.mov"

// ---------------------------------------------------------------- keys

func post(_ code: CGKeyCode, _ flags: CGEventFlags = []) {
    for down in [true, false] {
        let e = CGEvent(keyboardEventSource: nil, virtualKey: code, keyDown: down)!
        e.flags = flags
        e.post(tap: .cghidEventTap)
        usleep(12_000)
    }
}

func type(_ text: String, gap: Double = 0.085) {
    for unit in text.utf16 {
        for down in [true, false] {
            let e = CGEvent(keyboardEventSource: nil, virtualKey: 0, keyDown: down)!
            var c = unit
            e.keyboardSetUnicodeString(stringLength: 1, unicodeString: &c)
            e.post(tap: .cghidEventTap)
        }
        wait(gap)
    }
}

func wait(_ s: Double) { usleep(useconds_t(s * 1_000_000)) }
func summon() { post(49, .maskCommand) }          // Command-Space
func clear() { post(0, .maskCommand); post(51) }  // Command-A, Delete
func down() { post(125) }
func escape() { post(53) }
func actions() { post(40, .maskCommand) }         // Command-K

/// The demo itself. Each step leaves time for the springs to settle.
func perform() {
    wait(0.9)
    summon(); wait(0.5)
    type(" "); post(51); wait(0.6)            // a throwaway key, so none below is lost
    // The glass takes each app's colours from its icon.
    for app in ["spotify", "chrome", "safari", "maps", "photos"] {
        type(app, gap: 0.06); wait(1.25)
        clear(); wait(0.45)
    }
    type("s"); wait(0.9)                      // the list grows on its spring
    posterAt = Date().timeIntervalSince(recordingStarted) + 0.3
    for _ in 0..<3 { down(); wait(0.7) }      // and the colour follows the selection
    clear(); wait(0.9)                        // and shrinks back
    type("12*7"); wait(1.4)
    clear(); type("100 usd to inr"); wait(1.8)
    clear(); type("time in tokyo"); wait(1.7)
    clear(); type("5 km to mi"); wait(1.4)
    clear(); type("termi"); wait(0.9)
    actions(); wait(1.6)                      // Command-K actions
    escape(); wait(0.8)
    clear(); wait(0.8)
    summon(); wait(0.8)
}

// ---------------------------------------------------------------- backdrop

/// Flat colour shapes on a dark ground, drawn here so the GIF carries no
/// third-party artwork. Hard edges show the glass bending light, and flat
/// colour survives a GIF's 256-colour palette without banding.
final class Backdrop: NSView {
    override func draw(_ dirty: NSRect) {
        guard let ctx = NSGraphicsContext.current?.cgContext else { return }
        let b = bounds
        let c = { (r: CGFloat, g: CGFloat, bl: CGFloat) in CGColor(srgbRed: r, green: g, blue: bl, alpha: 1) }
        ctx.setFillColor(c(0.07, 0.08, 0.13))
        ctx.fill(b)
        // (x, y, radius) as fractions of the width, and a colour.
        let shapes: [(CGFloat, CGFloat, CGFloat, CGColor)] = [
            (0.12, 0.80, 0.22, c(1.00, 0.48, 0.20)),
            (0.86, 0.86, 0.20, c(0.56, 0.33, 0.98)),
            (0.50, 0.12, 0.26, c(0.10, 0.66, 0.84)),
            (0.06, 0.12, 0.17, c(0.94, 0.28, 0.55)),
            (0.95, 0.24, 0.16, c(0.22, 0.80, 0.52)),
            (0.52, 0.66, 0.09, c(0.98, 0.80, 0.22)),
        ]
        for (x, y, r, colour) in shapes {
            let radius = b.width * r
            ctx.setFillColor(colour)
            ctx.fillEllipse(in: CGRect(x: b.width * x - radius, y: b.height * y - radius, width: radius * 2, height: radius * 2))
        }
        // Thin diagonal lines across everything.
        ctx.setStrokeColor(CGColor(srgbRed: 1, green: 1, blue: 1, alpha: 0.10))
        ctx.setLineWidth(2)
        var x: CGFloat = -b.height
        while x < b.width {
            ctx.move(to: CGPoint(x: x, y: 0))
            ctx.addLine(to: CGPoint(x: x + b.height, y: b.height))
            x += 46
        }
        ctx.strokePath()
    }
}

// ---------------------------------------------------------------- MP4

/// Re-encode the screen recording as H.264 at a fixed bit rate, and save
/// one frame as the poster.
func export(from path: String, to mp4: String, poster: String, at seconds: Double) -> Bool {
    let asset = AVURLAsset(url: URL(fileURLWithPath: path))
    guard let track = asset.tracks(withMediaType: .video).first,
          let reader = try? AVAssetReader(asset: asset) else { return false }
    let size = track.naturalSize
    let (w, h) = (Int(size.width) / 2 * 2, Int(size.height) / 2 * 2) // H.264 wants even sizes
    let output = AVAssetReaderTrackOutput(track: track, outputSettings: [kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA])
    reader.add(output)
    try? FileManager.default.removeItem(atPath: mp4)
    guard let writer = try? AVAssetWriter(outputURL: URL(fileURLWithPath: mp4), fileType: .mp4) else { return false }
    let input = AVAssetWriterInput(mediaType: .video, outputSettings: [
        AVVideoCodecKey: AVVideoCodecType.h264,
        AVVideoWidthKey: w,
        AVVideoHeightKey: h,
        AVVideoCompressionPropertiesKey: [
            AVVideoAverageBitRateKey: bitrate,
            AVVideoProfileLevelKey: AVVideoProfileLevelH264HighAutoLevel,
            AVVideoMaxKeyFrameIntervalKey: 60,
        ],
    ])
    input.expectsMediaDataInRealTime = false
    writer.add(input)
    reader.startReading()
    writer.startWriting()
    var started = false
    while let sample = output.copyNextSampleBuffer() {
        if !started {
            writer.startSession(atSourceTime: CMSampleBufferGetPresentationTimeStamp(sample))
            started = true
        }
        while !input.isReadyForMoreMediaData { usleep(2_000) }
        input.append(sample)
    }
    input.markAsFinished()
    let done = DispatchSemaphore(value: 0)
    writer.finishWriting { done.signal() }
    done.wait()
    guard writer.status == .completed else { return false }

    let generator = AVAssetImageGenerator(asset: asset)
    generator.requestedTimeToleranceBefore = .zero
    generator.requestedTimeToleranceAfter = .zero
    if let image = try? generator.copyCGImage(at: CMTime(seconds: seconds, preferredTimescale: 600), actualTime: nil),
       let dest = CGImageDestinationCreateWithURL(URL(fileURLWithPath: poster) as CFURL, UTType.png.identifier as CFString, 1, nil) {
        CGImageDestinationAddImage(dest, image, nil)
        CGImageDestinationFinalize(dest)
    }
    return true
}

// ---------------------------------------------------------------- run

if CommandLine.arguments.contains("--encode-only") {
    let ok = FileManager.default.fileExists(atPath: movie) && export(from: movie, to: out, poster: NSTemporaryDirectory() + "unused-poster.png", at: 0)
    let bytes = (try? FileManager.default.attributesOfItem(atPath: out)[.size] as? Int) ?? 0
    print(ok ? "wrote \(out) (\(bytes / 1024) KB)" : "no recording to encode at \(movie)")
    exit(ok ? 0 : 1)
}

// The glass colour at "strong" for the recording, so the colour changes
// read in a small GIF; config.toml is put back exactly as it was after.
let configPath = NSHomeDirectory() + "/.config/rusty-spotlight/config.toml"
let savedConfig = try? String(contentsOfFile: configPath, encoding: .utf8)
if let text = savedConfig {
    var lines = text.components(separatedBy: "\n")
    if let i = lines.firstIndex(where: { $0.hasPrefix("theme_strength") }) {
        lines[i] = "theme_strength = \"strong\""
    } else if let i = lines.firstIndex(where: { $0.hasPrefix("theme ") || $0.hasPrefix("theme=") }) {
        lines.insert("theme_strength = \"strong\"", at: i + 1)
    }
    try? lines.joined(separator: "\n").write(toFile: configPath, atomically: true, encoding: .utf8)
}

let app = NSApplication.shared
// Never the active app: the panel must keep every key.
app.setActivationPolicy(.prohibited)
let screen = NSScreen.main!.frame
// The panel opens centred, its top 28% down the screen with the pointer.
// The 640 pt panel with room for its shadow, and its full height of rows.
let size = NSSize(width: 780, height: 600)
let topFromTop = screen.height * 0.28 - 44
let rect = NSRect(x: (screen.width - size.width) / 2, y: screen.height - topFromTop - size.height, width: size.width, height: size.height)
let window = NSWindow(contentRect: rect, styleMask: .borderless, backing: .buffered, defer: false)
window.level = .floating
window.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
window.contentView = Backdrop(frame: NSRect(origin: .zero, size: size))
window.orderFrontRegardless()

// Park the pointer on this screen, outside the recorded area.
let pointer = NSEvent.mouseLocation
CGWarpMouseCursorPosition(CGPoint(x: screen.width - 4, y: screen.height - 4))

DispatchQueue.global().async {
    wait(0.6)
    try? FileManager.default.removeItem(atPath: movie)
    let capture = Process()
    capture.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
    capture.arguments = ["-x", "-v", "-V", String(Int(seconds)),
                         "-R", "\(Int(rect.minX)),\(Int(topFromTop)),\(Int(size.width)),\(Int(size.height))", movie]
    try! capture.run()
    recordingStarted = Date()
    perform()
    capture.waitUntilExit()
    DispatchQueue.main.async {
        window.orderOut(nil)
        if let text = savedConfig {
            try? text.write(toFile: configPath, atomically: true, encoding: .utf8)
        }
        CGWarpMouseCursorPosition(CGPoint(x: pointer.x, y: screen.height - pointer.y))
        try? FileManager.default.createDirectory(atPath: "assets", withIntermediateDirectories: true)
        let ok = export(from: movie, to: out, poster: poster, at: posterAt)
        let bytes = (try? FileManager.default.attributesOfItem(atPath: out)[.size] as? Int) ?? 0
        print(ok ? "wrote \(out) (\(bytes / 1024) KB) and \(poster)" : "could not write the MP4; the recording is at \(movie)")
        exit(ok ? 0 : 1)
    }
}
app.run()
