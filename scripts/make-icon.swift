// Draws the app icon and writes assets/AppIcon.icns.
//
//   swift scripts/make-icon.swift            (from the repository root)
//
// A rust-orange squircle on Apple's icon grid (824 pt body on a 1024 pt
// canvas) with a glass magnifying lens. Every size is drawn from vectors,
// so small sizes stay sharp. Needs only the Xcode Command Line Tools.

import AppKit

let canvas: CGFloat = 1024
let bodyRect = CGRect(x: 100, y: 100, width: 824, height: 824)
let corner: CGFloat = 185

func colour(_ hex: UInt32, _ alpha: CGFloat = 1) -> CGColor {
    CGColor(srgbRed: CGFloat((hex >> 16) & 255) / 255, green: CGFloat((hex >> 8) & 255) / 255,
            blue: CGFloat(hex & 255) / 255, alpha: alpha)
}

func gradient(_ stops: [(UInt32, CGFloat, CGFloat)]) -> CGGradient {
    CGGradient(colorsSpace: CGColorSpace(name: CGColorSpace.sRGB),
               colors: stops.map { colour($0.0, $0.1) } as CFArray,
               locations: stops.map { $0.2 })!
}

func draw(_ ctx: CGContext) {
    let body = CGPath(roundedRect: bodyRect, cornerWidth: corner, cornerHeight: corner, transform: nil)

    // Drop shadow under the body, as macOS icons have.
    ctx.saveGState()
    ctx.setShadow(offset: CGSize(width: 0, height: -12), blur: 28, color: colour(0x000000, 0.35))
    ctx.addPath(body)
    ctx.setFillColor(colour(0xB4441A))
    ctx.fillPath()
    ctx.restoreGState()

    // Body: warm orange at the top left to deep rust at the bottom right.
    ctx.saveGState()
    ctx.addPath(body)
    ctx.clip()
    ctx.drawLinearGradient(gradient([(0xFF9A4D, 1, 0), (0xE8642A, 1, 0.45), (0x9C3412, 1, 1)]),
                           start: CGPoint(x: 160, y: 900), end: CGPoint(x: 880, y: 120), options: [])
    // A soft glow from above, like light on glass.
    ctx.drawRadialGradient(gradient([(0xFFFFFF, 0.28, 0), (0xFFFFFF, 0, 1)]),
                           startCenter: CGPoint(x: 400, y: 860), startRadius: 0,
                           endCenter: CGPoint(x: 400, y: 860), endRadius: 560, options: [])
    // A fine bright rim along the top edge.
    ctx.addPath(CGPath(roundedRect: bodyRect.insetBy(dx: 3, dy: 3), cornerWidth: corner - 3, cornerHeight: corner - 3, transform: nil))
    ctx.setStrokeColor(colour(0xFFFFFF, 0.22))
    ctx.setLineWidth(6)
    ctx.strokePath()
    ctx.restoreGState()

    // The magnifying glass.
    let centre = CGPoint(x: 462, y: 572)
    let radius: CGFloat = 196
    let ring: CGFloat = 58
    let angle = -CGFloat.pi / 4 // handle towards the bottom right
    let handleStart = CGPoint(x: centre.x + cos(angle) * (radius + ring * 0.35), y: centre.y + sin(angle) * (radius + ring * 0.35))
    let handleEnd = CGPoint(x: centre.x + cos(angle) * (radius + 250), y: centre.y + sin(angle) * (radius + 250))

    ctx.saveGState()
    ctx.setShadow(offset: CGSize(width: 0, height: -14), blur: 30, color: colour(0x5A1A05, 0.45))
    ctx.beginTransparencyLayer(auxiliaryInfo: nil)
    // Handle.
    ctx.setLineCap(.round)
    ctx.setLineWidth(86)
    ctx.setStrokeColor(colour(0xFFF4EC))
    ctx.move(to: handleStart)
    ctx.addLine(to: handleEnd)
    ctx.strokePath()
    // Lens: frosted glass inside the ring.
    let lens = CGRect(x: centre.x - radius, y: centre.y - radius, width: radius * 2, height: radius * 2)
    ctx.addEllipse(in: lens)
    ctx.setFillColor(colour(0xFFFFFF, 0.16))
    ctx.fillPath()
    // Ring.
    ctx.addEllipse(in: lens)
    ctx.setLineWidth(ring)
    ctx.setStrokeColor(colour(0xFFFFFF))
    ctx.strokePath()
    ctx.endTransparencyLayer()
    ctx.restoreGState()

    // Highlights on the glass: a crescent at the top left and a small glint.
    ctx.saveGState()
    ctx.addEllipse(in: lens.insetBy(dx: ring / 2, dy: ring / 2))
    ctx.clip()
    ctx.drawRadialGradient(gradient([(0xFFFFFF, 0.55, 0), (0xFFFFFF, 0, 1)]),
                           startCenter: CGPoint(x: centre.x - 70, y: centre.y + 80), startRadius: 0,
                           endCenter: CGPoint(x: centre.x - 70, y: centre.y + 80), endRadius: 190, options: [])
    ctx.restoreGState()
    ctx.setLineCap(.round)
    ctx.setLineWidth(22)
    ctx.setStrokeColor(colour(0xFFFFFF, 0.85))
    ctx.addArc(center: centre, radius: radius - 62, startAngle: .pi * 0.62, endAngle: .pi * 0.88, clockwise: false)
    ctx.strokePath()
    // A faint shade along the ring's lower edge gives it thickness.
    ctx.setLineWidth(10)
    ctx.setStrokeColor(colour(0xC9A48E, 0.6))
    ctx.addArc(center: centre, radius: radius + ring / 2 - 6, startAngle: -.pi * 0.95, endAngle: -.pi * 0.08, clockwise: false)
    ctx.strokePath()
}

func png(size: Int) -> Data {
    let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: size, pixelsHigh: size, bitsPerSample: 8,
                               samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB,
                               bytesPerRow: 0, bitsPerPixel: 0)!
    let ctx = NSGraphicsContext(bitmapImageRep: rep)!.cgContext
    ctx.scaleBy(x: CGFloat(size) / canvas, y: CGFloat(size) / canvas)
    draw(ctx)
    return rep.representation(using: .png, properties: [:])!
}

let fm = FileManager.default
let iconset = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("AppIcon.iconset")
try? fm.removeItem(at: iconset)
try! fm.createDirectory(at: iconset, withIntermediateDirectories: true)
for base in [16, 32, 128, 256, 512] {
    try! png(size: base).write(to: iconset.appendingPathComponent("icon_\(base)x\(base).png"))
    try! png(size: base * 2).write(to: iconset.appendingPathComponent("icon_\(base)x\(base)@2x.png"))
}
try! fm.createDirectory(atPath: "assets", withIntermediateDirectories: true)
try! png(size: 1024).write(to: URL(fileURLWithPath: "assets/AppIcon.png"))
let util = Process()
util.executableURL = URL(fileURLWithPath: "/usr/bin/iconutil")
util.arguments = ["-c", "icns", iconset.path, "-o", "assets/AppIcon.icns"]
try! util.run()
util.waitUntilExit()
print(util.terminationStatus == 0 ? "wrote assets/AppIcon.icns and assets/AppIcon.png" : "iconutil failed")
