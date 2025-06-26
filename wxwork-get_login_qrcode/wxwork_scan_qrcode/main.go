package main

import (
	"fmt"
	"image"
	_ "image/jpeg"
	_ "image/png"
	"io/fs"
	"log"
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"time"

	"github.com/makiuchi-d/gozxing"
	"github.com/makiuchi-d/gozxing/qrcode"
)

// QRScanner 二维码扫描器
type QRScanner struct {
	targetDir string
	interval  time.Duration
}

// NewQRScanner 创建扫描器
func NewQRScanner(targetDir string, interval time.Duration) *QRScanner {
	return &QRScanner{
		targetDir: targetDir,
		interval:  interval,
	}
}

// ScanOnce 执行一次扫描
func (qs *QRScanner) ScanOnce() error {
	latestFile, err := qs.getLatestQRCodeFile()
	if err != nil {
		return fmt.Errorf("获取最新文件失败: %v", err)
	}

	if latestFile == "" {
		fmt.Println("📂 未找到二维码文件")
		return nil
	}

	fmt.Printf("📄 最新文件: %s\n", filepath.Base(latestFile))

	// 检测二维码
	isQR, content, err := qs.decodeQRCode(latestFile)
	if err != nil {
		return fmt.Errorf("检测二维码失败: %v", err)
	}

	if isQR {
		fmt.Printf("✅ 发现二维码!\n")
		fmt.Printf("📱 内容: %s\n", content)
		fmt.Printf("⏰ 时间: %s\n", time.Now().Format("2006-01-02 15:04:05"))
	} else {
		fmt.Printf("❌ 非二维码文件\n")
	}

	return nil
}

// StartPolling 开始轮询扫描
func (qs *QRScanner) StartPolling() {
	fmt.Printf("🔄 开始轮询扫描，间隔: %v\n", qs.interval)
	fmt.Printf("📁 监控目录: %s\n", qs.targetDir)

	ticker := time.NewTicker(qs.interval)
	defer ticker.Stop()

	// 立即执行一次
	if err := qs.ScanOnce(); err != nil {
		log.Printf("扫描失败: %v", err)
	}

	// 定时扫描
	for range ticker.C {
		if err := qs.ScanOnce(); err != nil {
			log.Printf("扫描失败: %v", err)
		}
	}
}

// getLatestQRCodeFile 获取最新的二维码文件
func (qs *QRScanner) getLatestQRCodeFile() (string, error) {
	var latestFile string
	var latestTime time.Time

	// 检查目录是否存在
	if _, err := os.Stat(qs.targetDir); os.IsNotExist(err) {
		return "", fmt.Errorf("目录不存在: %s", qs.targetDir)
	}

	err := filepath.WalkDir(qs.targetDir, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}

		// 跳过目录
		if d.IsDir() {
			return nil
		}

		// 检查文件格式
		if !qs.isValidQRCodeFile(path) {
			return nil
		}

		info, err := d.Info()
		if err != nil {
			return err
		}

		// 比较修改时间
		if info.ModTime().After(latestTime) {
			latestTime = info.ModTime()
			latestFile = path
		}

		return nil
	})

	return latestFile, err
}

// isValidQRCodeFile 检查是否为有效的二维码文件
func (qs *QRScanner) isValidQRCodeFile(filePath string) bool {
	// 检查扩展名
	ext := strings.ToLower(filepath.Ext(filePath))
	if ext != ".jpg" {
		return false
	}

	// 检查UUID格式文件名
	filename := filepath.Base(filePath)
	filenameWithoutExt := strings.TrimSuffix(filename, filepath.Ext(filename))
	matched, _ := regexp.MatchString(`^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$`, filenameWithoutExt)
	return matched
}

// decodeQRCode 解码二维码
func (qs *QRScanner) decodeQRCode(imagePath string) (bool, string, error) {
	file, err := os.Open(imagePath)
	if err != nil {
		return false, "", fmt.Errorf("打开文件失败: %v", err)
	}
	defer file.Close()

	img, _, err := image.Decode(file)
	if err != nil {
		return false, "", fmt.Errorf("解码图片失败: %v", err)
	}

	reader := qrcode.NewQRCodeReader()
	bmp, err := gozxing.NewBinaryBitmapFromImage(img)
	if err != nil {
		return false, "", fmt.Errorf("转换图片失败: %v", err)
	}

	result, err := reader.Decode(bmp, nil)
	if err != nil {
		return false, "", nil // 不是二维码
	}

	return true, result.GetText(), nil
}

// getDefaultWXWorkPath 获取默认路径
func getDefaultWXWorkPath() string {
	userProfile := os.Getenv("USERPROFILE")
	if userProfile == "" {
		return `C:\Users\Default\Documents\WXWork\Global\Image`
	}
	return filepath.Join(userProfile, "Documents", "WXWork", "Global", "Image")
}

func main() {
	// 使用默认路径或自定义路径
	targetDir := getDefaultWXWorkPath()

	// 可以通过命令行参数自定义路径
	if len(os.Args) > 1 {
		targetDir = os.Args[1]
	}

	fmt.Println("🚀 企业微信二维码扫描器（定时模式）")
	fmt.Printf("📁 扫描目录: %s\n", targetDir)

	// 创建扫描器（每5秒扫描一次）
	scanner := NewQRScanner(targetDir, 5*time.Second)

	// 开始轮询
	scanner.StartPolling()
}
