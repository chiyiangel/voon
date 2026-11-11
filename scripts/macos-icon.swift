// A reproducible vector-drawn app icon using the prototype's terracotta palette.
import AppKit

guard CommandLine.arguments.count == 2 else { fatalError("需要图标输出目录") }
let iconset = URL(fileURLWithPath: CommandLine.arguments[1]).appendingPathComponent("Voon.iconset")
try FileManager.default.createDirectory(at: iconset, withIntermediateDirectories: true)

func render(size: Int, filename: String) throws {
    let bitmap = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: size, pixelsHigh: size,
        bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
        colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: bitmap)
    let scale = CGFloat(size) / 1024
    let transform = AffineTransform(scale: scale)
    (transform as NSAffineTransform).concat()

    let tile = NSBezierPath(roundedRect: NSRect(x: 64, y: 64, width: 896, height: 896), xRadius: 200, yRadius: 200)
    let gradient = NSGradient(starting: NSColor(srgbRed: 0.88, green: 0.51, blue: 0.34, alpha: 1),
                              ending: NSColor(srgbRed: 0.43, green: 0.18, blue: 0.10, alpha: 1))!
    gradient.draw(in: tile, angle: -60)
    NSColor(srgbRed: 1, green: 0.97, blue: 0.91, alpha: 1).setFill()
    let beam = NSBezierPath()
    beam.move(to: NSPoint(x: 370, y: 620))
    beam.line(to: NSPoint(x: 705, y: 700))
    beam.line(to: NSPoint(x: 705, y: 616))
    beam.line(to: NSPoint(x: 370, y: 536))
    beam.close()
    beam.fill()
    NSBezierPath(roundedRect: NSRect(x: 370, y: 329, width: 53, height: 292), xRadius: 12, yRadius: 12).fill()
    NSBezierPath(roundedRect: NSRect(x: 652, y: 392, width: 53, height: 276), xRadius: 12, yRadius: 12).fill()
    NSBezierPath(ovalIn: NSRect(x: 250, y: 265, width: 173, height: 120)).fill()
    NSBezierPath(ovalIn: NSRect(x: 532, y: 328, width: 173, height: 120)).fill()
    NSGraphicsContext.restoreGraphicsState()
    try bitmap.representation(using: .png, properties: [:])!.write(to: iconset.appendingPathComponent(filename))
}

for points in [16, 32, 128, 256, 512] {
    try render(size: points, filename: "icon_\(points)x\(points).png")
    try render(size: points * 2, filename: "icon_\(points)x\(points)@2x.png")
}
