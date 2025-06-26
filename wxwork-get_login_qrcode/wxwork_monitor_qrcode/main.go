package main

import (
	"fmt"
	"image"
	_ "image/jpeg"
	_ "image/png"
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"sync"
	"time"

	"github.com/fsnotify/fsnotify"
	"github.com/makiuchi-d/gozxing"
	"github.com/makiuchi-d/gozxing/qrcode"
)

// FileCache 文件缓存，避免重复处理
type FileCache struct {
	processed map[string]time.Time
	mutex     sync.RWMutex
}

// QRCodeMonitor 二维码监控器
type QRCodeMonitor struct {
	cache   *FileCache
	watcher *fsnotify.Watcher
}

// NewQRCodeMonitor 创建新的监控器
func NewQRCodeMonitor() (*QRCodeMonitor, error) {
	// 创建文件监控器
	watcher, err := fsnotify.NewWatcher()
	if err != nil {
		return nil, fmt.Errorf("创建文件监控器失败: %v", err)
	}

	return &QRCodeMonitor{
		cache: &FileCache{
			processed: make(map[string]time.Time),
		},
		watcher: watcher,
	}, nil
}

// getDefaultWXWorkPath 获取默认企业微信路径
func getDefaultWXWorkPath() string {
	userProfile := os.Getenv("USERPROFILE")
	if userProfile == "" {
		return `C:\Users\Default\Documents\WXWork\Global`
	}
	return filepath.Join(userProfile, "Documents", "WXWork", "Global")
}

// Start 开始监控
func (qm *QRCodeMonitor) Start() error {
	defer qm.watcher.Close()

	parentDir := getDefaultWXWorkPath()
	targetDir := filepath.Join(parentDir, "Image")

	fmt.Printf("开始监控企业微信二维码文件...\n")
	fmt.Printf("监控路径: %s\n", targetDir)

	// 检查并添加监控目录
	if err := qm.setupWatcher(parentDir, targetDir); err != nil {
		return fmt.Errorf("设置监控失败: %v", err)
	}

	// 启动监控循环
	return qm.watchLoop()
}

// setupWatcher 设置监控器
func (qm *QRCodeMonitor) setupWatcher(parentDir, targetDir string) error {
	if _, err := os.Stat(targetDir); err == nil {
		// Image目录已存在，直接监控
		fmt.Printf("✅ Image目录已存在，开始监控\n")
		return qm.watcher.Add(targetDir)
	} else {
		// Image目录不存在，监控上级目录
		fmt.Printf("⏳ Image目录不存在，监控上级目录等待创建\n")
		return qm.watcher.Add(parentDir)
	}
}

// watchLoop 监控循环
func (qm *QRCodeMonitor) watchLoop() error {
	for {
		select {
		case event, ok := <-qm.watcher.Events:
			if !ok {
				return nil
			}
			qm.handleEvent(event)

		case err, ok := <-qm.watcher.Errors:
			if !ok {
				return nil
			}
			fmt.Printf("❌ 监控错误: %v\n", err)
		}
	}
}

// handleEvent 处理文件系统事件
func (qm *QRCodeMonitor) handleEvent(event fsnotify.Event) {
	// 处理目录创建
	if event.Op&fsnotify.Create == fsnotify.Create {
		if filepath.Base(event.Name) == "Image" {
			fmt.Printf("\n✅ 检测到Image目录创建: %s\n", event.Name)
			if err := qm.watcher.Add(event.Name); err != nil {
				fmt.Printf("❌ 添加Image目录监控失败: %v\n", err)
			} else {
				fmt.Println("📁 Image目录监控添加成功")
			}
			return
		}
	}

	// 处理文件写入
	if event.Op&fsnotify.Write == fsnotify.Write {
		if qm.isValidQRCodeFile(event.Name) {
			// 检查是否已处理过
			fileInfo, err := os.Stat(event.Name)
			if err != nil {
				return
			}

			if qm.cache.isProcessed(event.Name, fileInfo.ModTime()) {
				return
			}

			fmt.Printf("\n📄 检测到新文件: %s\n", filepath.Base(event.Name))

			// 等待文件写入完成
			time.Sleep(500 * time.Millisecond)

			// 检测二维码
			qm.processQRCodeFile(event.Name)

			// 标记为已处理
			qm.cache.markProcessed(event.Name, fileInfo.ModTime())
		}
	}
}

// isProcessed 检查文件是否已处理
func (fc *FileCache) isProcessed(filePath string, modTime time.Time) bool {
	fc.mutex.RLock()
	defer fc.mutex.RUnlock()

	lastProcessed, exists := fc.processed[filePath]
	return exists && !modTime.After(lastProcessed)
}

// markProcessed 标记文件为已处理
func (fc *FileCache) markProcessed(filePath string, modTime time.Time) {
	fc.mutex.Lock()
	defer fc.mutex.Unlock()
	fc.processed[filePath] = modTime
}

// isValidQRCodeFile 检查文件是否符合二维码文件格式
func (qm *QRCodeMonitor) isValidQRCodeFile(filePath string) bool {
	// 检查文件扩展名
	ext := strings.ToLower(filepath.Ext(filePath))
	if ext != ".jpg" {
		return false
	}

	// 检查文件名格式：UUID格式
	filename := filepath.Base(filePath)
	filenameWithoutExt := strings.TrimSuffix(filename, filepath.Ext(filename))
	// 匹配UUID格式：8-4-4-4-12位十六进制字符
	matched, _ := regexp.MatchString(`^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$`, filenameWithoutExt)
	return matched
}

// processQRCodeFile 处理二维码文件
func (qm *QRCodeMonitor) processQRCodeFile(imagePath string) {
	// 验证文件完整性
	if err := qm.validateFile(imagePath); err != nil {
		fmt.Printf("❌ 文件验证失败: %v\n", err)
		return
	}

	// 检测二维码
	isQR, content, err := qm.decodeQRCode(imagePath)
	if err != nil {
		fmt.Printf("❌ 检测二维码失败: %v\n", err)
		return
	}

	if isQR {
		fmt.Printf("✅ 发现二维码!\n")
		fmt.Printf("📱 内容: %s\n", content)
		fmt.Printf("⏰ 时间: %s\n", time.Now().Format("2006-01-02 15:04:05"))
		fmt.Printf("📁 文件: %s\n", filepath.Base(imagePath))

	} else {
		fmt.Printf("❌ 非二维码文件\n")
	}

	fmt.Println("📡 继续监控中...")
}

// validateFile 验证文件完整性
func (qm *QRCodeMonitor) validateFile(imagePath string) error {
	// 检查文件是否存在
	fileInfo, err := os.Stat(imagePath)
	if os.IsNotExist(err) {
		return fmt.Errorf("文件不存在: %s", imagePath)
	}
	if err != nil {
		return fmt.Errorf("获取文件信息失败: %v", err)
	}

	// 检查文件大小
	if fileInfo.Size() == 0 {
		return fmt.Errorf("文件为空")
	}

	// 检查文件大小是否合理（避免处理过大的文件）
	if fileInfo.Size() > 10*1024*1024 { // 10MB
		return fmt.Errorf("文件过大: %d bytes", fileInfo.Size())
	}

	return nil
}

// decodeQRCode 解码二维码
func (qm *QRCodeMonitor) decodeQRCode(imagePath string) (bool, string, error) {
	// 打开图片文件
	file, err := os.Open(imagePath)
	if err != nil {
		return false, "", fmt.Errorf("打开文件失败: %v", err)
	}
	defer file.Close()

	// 解码图片
	img, _, err := image.Decode(file)
	if err != nil {
		return false, "", fmt.Errorf("解码图片失败: %v", err)
	}

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

// main 主函数
func main() {
	// 创建监控器
	monitor, err := NewQRCodeMonitor()
	if err != nil {
		fmt.Printf("❌ 创建监控器失败: %v\n", err)
		os.Exit(1)
	}

	// 开始监控
	fmt.Println("🚀 企业微信二维码监控器启动")
	if err := monitor.Start(); err != nil {
		fmt.Printf("❌ 监控失败: %v\n", err)
		os.Exit(1)
	}
}
