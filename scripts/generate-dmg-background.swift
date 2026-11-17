import AppKit

// Render at 2× resolution; the bitmap's logical size matches the Finder window.
// Icon positions are configured in src-tauri/tauri.conf.json, not painted here.
let width = 660
let height = 420
let scale = 2
let bitmap = NSBitmapImageRep(
    bitmapDataPlanes: nil,
    pixelsWide: width * scale,
    pixelsHigh: height * scale,
    bitsPerSample: 8,
    samplesPerPixel: 4,
    hasAlpha: true,
    isPlanar: false,
    colorSpaceName: .deviceRGB,
    bytesPerRow: 0,
    bitsPerPixel: 0
)!
let context = NSGraphicsContext(bitmapImageRep: bitmap)!
context.cgContext.scaleBy(x: CGFloat(scale), y: CGFloat(scale))
context.cgContext.translateBy(x: 0, y: CGFloat(height))
context.cgContext.scaleBy(x: 1, y: -1)
NSGraphicsContext.current = NSGraphicsContext(cgContext: context.cgContext, flipped: true)

func color(_ red: CGFloat, _ green: CGFloat, _ blue: CGFloat) -> NSColor {
    NSColor(srgbRed: red / 255, green: green / 255, blue: blue / 255, alpha: 1)
}

func text(_ value: String, y: CGFloat, size: CGFloat, weight: NSFont.Weight, ink: NSColor) {
    let paragraph = NSMutableParagraphStyle()
    paragraph.alignment = .center
    let attributes: [NSAttributedString.Key: Any] = [
        .font: NSFont.systemFont(ofSize: size, weight: weight),
        .foregroundColor: ink,
        .paragraphStyle: paragraph,
    ]
    (value as NSString).draw(
        in: NSRect(x: 24, y: y, width: CGFloat(width) - 48, height: size * 1.8),
        withAttributes: attributes
    )
}

color(247, 249, 253).setFill()
NSBezierPath(rect: NSRect(x: 0, y: 0, width: width, height: height)).fill()

text("安装项目工作台", y: 48, size: 26, weight: .semibold, ink: color(29, 39, 59))
text("拖入应用程序，即可完成安装", y: 89, size: 14, weight: .regular, ink: color(109, 121, 142))

// Subtle wells frame the real, draggable Finder icons.
for x in [180, 480] {
    NSColor.white.setFill()
    let well = NSBezierPath(roundedRect: NSRect(x: x - 64, y: 151, width: 128, height: 128), xRadius: 28, yRadius: 28)
    well.fill()
    color(231, 237, 247).setStroke()
    well.lineWidth = 1
    well.stroke()
}

color(22, 119, 255).setStroke()
let arrow = NSBezierPath()
arrow.move(to: NSPoint(x: 296, y: 215))
arrow.line(to: NSPoint(x: 364, y: 215))
arrow.move(to: NSPoint(x: 351, y: 202))
arrow.line(to: NSPoint(x: 364, y: 215))
arrow.line(to: NSPoint(x: 351, y: 228))
arrow.lineWidth = 3
arrow.lineCapStyle = .round
arrow.lineJoinStyle = .round
arrow.stroke()

color(225, 232, 242).setStroke()
let divider = NSBezierPath()
divider.move(to: NSPoint(x: 48, y: 326))
divider.line(to: NSPoint(x: 612, y: 326))
divider.lineWidth = 1
divider.stroke()
text("将左侧应用拖到右侧「应用程序」文件夹", y: 347, size: 14, weight: .medium, ink: color(72, 87, 112))
text("安装完成后，从「应用程序」打开项目工作台", y: 373, size: 12, weight: .regular, ink: color(129, 141, 160))

NSGraphicsContext.current = nil
bitmap.size = NSSize(width: width, height: height)

let output = URL(fileURLWithPath: #filePath)
    .deletingLastPathComponent().deletingLastPathComponent()
    .appendingPathComponent("src-tauri/dmg/background.png")
try FileManager.default.createDirectory(at: output.deletingLastPathComponent(), withIntermediateDirectories: true)
try bitmap.representation(using: .png, properties: [:])!.write(to: output)
print("已生成 \(output.path)")
