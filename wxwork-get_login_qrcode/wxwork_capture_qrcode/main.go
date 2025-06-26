package main

import (
	"fmt"
	"image"
	"image/color"
	"image/png"
	"log"
	"os"
	"syscall"
	"unsafe"

	"github.com/makiuchi-d/gozxing"
	"github.com/makiuchi-d/gozxing/qrcode"
)

// Windows API 常量
const (
	DIB_RGB_COLORS       = 0
	SRCCOPY              = 0x00CC0020
	PW_CLIENTONLY        = 0x00000001
	PW_RENDERFULLCONTENT = 0x00000002
)

// Windows API 结构体
type RECT struct {
	Left, Top, Right, Bottom int32
}

type BITMAPINFOHEADER struct {
	BiSize          uint32
	BiWidth         int32
	BiHeight        int32
	BiPlanes        uint16
	BiBitCount      uint16
	BiCompression   uint32
	BiSizeImage     uint32
	BiXPelsPerMeter int32
	BiYPelsPerMeter int32
	BiClrUsed       uint32
	BiClrImportant  uint32
}

type BITMAPINFO struct {
	BmiHeader BITMAPINFOHEADER
	BmiColors [1]uint32
}

// Windows API 函数
var (
	user32                 = syscall.NewLazyDLL("user32.dll")
	gdi32                  = syscall.NewLazyDLL("gdi32.dll")
	shcore                 = syscall.NewLazyDLL("shcore.dll")
	findWindowW            = user32.NewProc("FindWindowW")
	getWindowRect          = user32.NewProc("GetWindowRect")
	getDC                  = user32.NewProc("GetDC")
	releaseDC              = user32.NewProc("ReleaseDC")
	createCompatibleDC     = gdi32.NewProc("CreateCompatibleDC")
	createCompatibleBitmap = gdi32.NewProc("CreateCompatibleBitmap")
	selectObject           = gdi32.NewProc("SelectObject")
	printWindow            = user32.NewProc("PrintWindow")
	getDIBits              = gdi32.NewProc("GetDIBits")
	deleteDC               = gdi32.NewProc("DeleteDC")
	deleteObject           = gdi32.NewProc("DeleteObject")
	getDpiForWindow        = user32.NewProc("GetDpiForWindow")
	setProcessDpiAwareness = shcore.NewProc("SetProcessDpiAwareness")
)

// WindowCapture 窗口截图结构体
type WindowCapture struct {
	hwnd uintptr
}

// NewWindowCapture 创建窗口截图实例
func NewWindowCapture() *WindowCapture {
	// 设置DPI感知
	setProcessDpiAwareness.Call(uintptr(2)) // PROCESS_PER_MONITOR_DPI_AWARE
	return &WindowCapture{}
}

// FindWindow 查找窗口
func (wc *WindowCapture) FindWindow(className, windowName string) error {
	var classNamePtr, windowNamePtr uintptr

	if className != "" {
		classNameUTF16, _ := syscall.UTF16PtrFromString(className)
		classNamePtr = uintptr(unsafe.Pointer(classNameUTF16))
	}

	if windowName != "" {
		windowNameUTF16, _ := syscall.UTF16PtrFromString(windowName)
		windowNamePtr = uintptr(unsafe.Pointer(windowNameUTF16))
	}

	hwnd, _, _ := findWindowW.Call(classNamePtr, windowNamePtr)
	if hwnd == 0 {
		return fmt.Errorf("未找到窗口: className=%s, windowName=%s", className, windowName)
	}

	wc.hwnd = hwnd
	fmt.Printf("找到窗口句柄: %d\n", hwnd)
	return nil
}

// GetWindowRect 获取窗口矩形
func (wc *WindowCapture) GetWindowRect() (RECT, error) {
	var rect RECT
	ret, _, _ := getWindowRect.Call(wc.hwnd, uintptr(unsafe.Pointer(&rect)))
	if ret == 0 {
		return rect, fmt.Errorf("获取窗口矩形失败")
	}
	return rect, nil
}

// GetDpiScale 获取DPI缩放比例
func (wc *WindowCapture) GetDpiScale() float64 {
	dpi, _, _ := getDpiForWindow.Call(wc.hwnd)
	if dpi == 0 {
		return 1.0 // 默认96 DPI
	}
	return float64(dpi) / 96.0
}

// CaptureWindow 截取窗口
func (wc *WindowCapture) CaptureWindow(filename string) error {
	if wc.hwnd == 0 {
		return fmt.Errorf("窗口句柄无效")
	}

	// 获取窗口矩形
	rect, err := wc.GetWindowRect()
	if err != nil {
		return fmt.Errorf("获取窗口矩形失败: %v", err)
	}

	width := int(rect.Right - rect.Left)
	height := int(rect.Bottom - rect.Top)

	fmt.Printf("窗口尺寸: %dx%d\n", width, height)

	// 获取DPI缩放
	// dpiScale := wc.GetDpiScale()
	// fmt.Printf("DPI缩放: %.2f\n", dpiScale)

	// 调整尺寸以适应DPI缩放
	// actualWidth := int(float64(width) * dpiScale)
	// actualHeight := int(float64(height) * dpiScale)
	actualWidth := width
	actualHeight := height

	// 获取窗口DC
	windowDC, _, _ := getDC.Call(wc.hwnd)
	if windowDC == 0 {
		return fmt.Errorf("获取窗口DC失败")
	}
	defer releaseDC.Call(wc.hwnd, windowDC)

	// 创建兼容DC
	memDC, _, _ := createCompatibleDC.Call(windowDC)
	if memDC == 0 {
		return fmt.Errorf("创建兼容DC失败")
	}
	defer deleteDC.Call(memDC)

	// 创建兼容位图
	hBitmap, _, _ := createCompatibleBitmap.Call(windowDC, uintptr(actualWidth), uintptr(actualHeight))
	if hBitmap == 0 {
		return fmt.Errorf("创建兼容位图失败")
	}
	defer deleteObject.Call(hBitmap)

	// 选择位图到DC
	oldBitmap, _, _ := selectObject.Call(memDC, hBitmap)
	defer selectObject.Call(memDC, oldBitmap)

	// 使用PrintWindow截取窗口
	ret, _, _ := printWindow.Call(wc.hwnd, memDC, PW_RENDERFULLCONTENT)
	if ret == 0 {
		return fmt.Errorf("PrintWindow失败")
	}

	// 创建BITMAPINFO结构
	bi := BITMAPINFO{
		BmiHeader: BITMAPINFOHEADER{
			BiSize:        uint32(unsafe.Sizeof(BITMAPINFOHEADER{})),
			BiWidth:       int32(actualWidth),
			BiHeight:      -int32(actualHeight), // 负值表示自顶向下
			BiPlanes:      1,
			BiBitCount:    32, // 32位RGBA
			BiCompression: 0,  // BI_RGB
		},
	}

	// 计算图像数据大小
	imageSize := actualWidth * actualHeight * 4 // 32位 = 4字节
	imageData := make([]byte, imageSize)

	// 获取位图数据
	ret, _, _ = getDIBits.Call(
		windowDC,
		hBitmap,
		0,
		uintptr(actualHeight),
		uintptr(unsafe.Pointer(&imageData[0])),
		uintptr(unsafe.Pointer(&bi)),
		DIB_RGB_COLORS,
	)

	if ret == 0 {
		return fmt.Errorf("获取位图数据失败")
	}

	// 转换为Go image.Image
	img := wc.convertToImage(imageData, actualWidth, actualHeight)

	// 解析二维码
	hasQR, qrContent, err := wc.decodeQRCode(img)
	if err != nil {
		fmt.Printf("❌ 二维码解析出错: %v\n", err)
	} else if hasQR {
		fmt.Printf("✅ 发现二维码内容: %s\n", qrContent)
	} else {
		fmt.Printf("ℹ️ 未在截图中发现二维码\n")
	}

	// 保存图像
	return wc.saveImage(img, filename)
}

// convertToImage 将BGRA数据转换为image.Image
func (wc *WindowCapture) convertToImage(data []byte, width, height int) image.Image {
	img := image.NewRGBA(image.Rect(0, 0, width, height))

	for y := 0; y < height; y++ {
		for x := 0; x < width; x++ {
			offset := (y*width + x) * 4
			// Windows位图格式是BGRA，需要转换为RGBA
			b := data[offset]
			g := data[offset+1]
			r := data[offset+2]
			a := data[offset+3]

			img.Set(x, y, color.RGBA{R: r, G: g, B: b, A: a})
		}
	}

	return img
}

// saveImage 保存图像到文件
func (wc *WindowCapture) saveImage(img image.Image, filename string) error {
	file, err := os.Create(filename)
	if err != nil {
		return fmt.Errorf("创建文件失败: %v", err)
	}
	defer file.Close()

	err = png.Encode(file, img)
	if err != nil {
		return fmt.Errorf("编码PNG失败: %v", err)
	}

	fmt.Printf("截图已保存到: %s\n", filename)
	return nil
}

// decodeQRCode 解析图像中的二维码
func (wc *WindowCapture) decodeQRCode(img image.Image) (bool, string, error) {
	// 创建二维码读取器
	reader := qrcode.NewQRCodeReader()

	// 将图片转换为BinaryBitmap
	bmp, err := gozxing.NewBinaryBitmapFromImage(img)
	if err != nil {
		return false, "", fmt.Errorf("转换图片失败: %v", err)
	}

	// 尝试解码二维码
	result, err := reader.Decode(bmp, nil)
	if err != nil {
		// 解码失败，可能不是二维码
		return false, "", nil
	}

	return true, result.GetText(), nil
}

func main() {

	className := "WeChatLogin"
	windowName := "企业微信"
	outputFile := "screenshot.png"

	// 如果类名是空字符串，则设为空
	if className == "" {
		className = ""
	}

	fmt.Printf("正在查找窗口...\n")
	fmt.Printf("类名: %s\n", className)
	fmt.Printf("窗口名: %s\n", windowName)

	// 创建窗口截图实例
	capture := NewWindowCapture()

	// 查找窗口
	err := capture.FindWindow(className, windowName)
	if err != nil {
		log.Fatalf("查找窗口失败: %v", err)
	}

	// 截取窗口
	err = capture.CaptureWindow(outputFile)
	if err != nil {
		log.Fatalf("截取窗口失败: %v", err)
	}

	fmt.Println("截图完成!")
}
