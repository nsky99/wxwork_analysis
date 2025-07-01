// dllmain.cpp : 定义 DLL 应用程序的入口点。
#include "pch.h"

#include <windows.h>
#include <detours/detours.h>
#pragma comment(lib,"detours.lib")
#include <string>
#include <iostream>
#include <sstream>
#include <fstream>
#include <filesystem>
#include <direct.h>
#include <locale>
#include <codecvt>

// 定义 LoadXML 函数的签名
// 根据你的描述：std::wstring* DuiLib::CResManager::LoadXML(void*, wchar_t const*, int)
using LoadXMLFn = std::wstring* (__cdecl*)(void*, const wchar_t*, int);

// 定义WriteLog 函数签名
using WriteLogFn = void* (__cdecl*)(void* _this,  std::string* log);


// 保存原始函数指针
static LoadXMLFn OriginalLoadXML = nullptr;

static WriteLogFn OriginalWriteLog = nullptr;

// Hook 函数实现
std::wstring* __cdecl LoadXMLProxy(void* _this, const wchar_t* path, int flag) {
    // 打印输入参数
    std::wstringstream wss;
    if (path != nullptr) {
        wss << L"duilib_helper LoadXML called: path=" << path << L", flag=" << flag;
    }
    else {
        wss << L"duilib_helper LoadXML called: path=NULL, flag=" << flag;
    }
    OutputDebugStringW(wss.str().c_str());

    // 调用原始函数
    std::wstring* result = OriginalLoadXML(_this, path, flag);



    // 保存XML内容到文件
    if (result != nullptr && path != nullptr) {
        try {
            // 在路径前追加wxwork_ui目录
            std::wstring newPath = L"wxwork_ui\\";
            newPath += path;
            
            // 创建目录结构
            std::wstring dirPath = newPath;
            size_t lastSlash = dirPath.find_last_of(L"\\/");
            
            // 如果路径中包含目录部分
            if (lastSlash != std::wstring::npos) {
                std::wstring directory = dirPath.substr(0, lastSlash);
                
                // 创建目录（如果不存在）
                if (!directory.empty()) {
                    std::error_code ec;
                    std::filesystem::create_directories(directory, ec);
                    
                    if (ec) {
                        std::wstringstream wss2;
                        wss2 << L"duilib_helper Failed to create directory: " << directory << L", error: " << ec.message().c_str();
                        OutputDebugStringW(wss2.str().c_str());
                        return result;
                    }
                }
            }
            
            // 保存XML内容到文件 - 使用UTF-8编码
            std::ofstream file(newPath, std::ios::binary);
            if (file.is_open()) {
                // 设置UTF-8 BOM
                const char bom[] = { (char)0xEF, (char)0xBB, (char)0xBF };
                file.write(bom, 3);
                
                // 将宽字符串转换为UTF-8
                std::wstring_convert<std::codecvt_utf8<wchar_t>> converter;
                std::string utf8Content = converter.to_bytes(result->c_str());
                
                // 写入UTF-8内容
                file << utf8Content;
                file.close();
                
                std::wstringstream wss2;
                wss2 << L"duilib_helper XML content saved to file (UTF-8): " << newPath << L" (original path: " << path << L")";
                OutputDebugStringW(wss2.str().c_str());
            }
            else {
                std::wstringstream wss2;
                wss2 << L"duilib_helper Failed to open file for writing: " << newPath;
                OutputDebugStringW(wss2.str().c_str());
            }
        }
        catch (const std::exception& e) {
            std::wstringstream wss2;
            wss2 << L"duilib_helper Exception while saving XML: " << e.what();
            OutputDebugStringW(wss2.str().c_str());
        }
    }
    else if (path != nullptr) {
        std::wstringstream wss2;
        wss2 << L"duilib_helper Cannot save XML: result is NULL for path " << path;
        OutputDebugStringW(wss2.str().c_str());
    }

    return result;
}

void* __cdecl WriteLogProxy(void* _this, std::string *log) {
    // 写日志
    std::stringstream ss;
    ss << "wxwork_log\t" << log->c_str();
    OutputDebugStringA(ss.str().c_str());
    return OriginalWriteLog(_this, log);
}

void hook_load_xml() {
    // 1. 加载 duilib.dll
    HMODULE hDll = LoadLibraryW(L"duilib.dll");
    if (hDll == nullptr) {
        std::wstringstream wss;
        wss << L"LoadLibraryW failed: " << GetLastError();
        OutputDebugStringW(wss.str().c_str());
        return;
    }

    // 2. 获取 LoadXML 函数地址
    FARPROC addr = GetProcAddress(
        hDll,
        "?LoadXML@CResManager@DuiLib@@SA?AV?$basic_string@_WU?$char_traits@_W@std@@V?$allocator@_W@2@@std@@PB_WH@Z"
    );
    if (addr == nullptr) {
        std::wstringstream wss;
        wss << L"GetProcAddress failed: " << GetLastError();
        OutputDebugStringW(wss.str().c_str());
        FreeLibrary(hDll);
        return;
    }

    // 3. 保存原始函数并设置 hook
    OriginalLoadXML = reinterpret_cast<LoadXMLFn>(addr);

    // 开始 Detours 事务
    DetourTransactionBegin();
    DetourUpdateThread(GetCurrentThread());
    DetourAttach(&(PVOID&)OriginalLoadXML, LoadXMLProxy);

    // 提交事务
    LONG error = DetourTransactionCommit();
    if (error != NO_ERROR) {
        std::wstringstream wss;
        wss << L"Failed to set hook: " << error;
        OutputDebugStringW(wss.str().c_str());
        FreeLibrary(hDll);
        return;
    }

    OutputDebugStringW(L"Successfully hooked LoadXML function");
}

void hook_write_log() {
    // 1. 获取模块基地址
    HMODULE hDll = LoadLibraryW(L"wxwork.exe");
    if (hDll == nullptr) {
        std::wstringstream wss;
        wss << L"LoadLibraryW failed: " << GetLastError();
        OutputDebugStringW(wss.str().c_str());
        return;
    }

    // 2. 获取 write_log 函数地址
    FARPROC addr = FARPROC(DWORD(hDll) + 0x33D158);

    // 3. 保存原始函数并设置 hook
    OriginalWriteLog = reinterpret_cast<WriteLogFn>(addr);

    // 开始 Detours 事务
    DetourTransactionBegin();
    DetourUpdateThread(GetCurrentThread());
    DetourAttach(&(PVOID&)OriginalWriteLog, WriteLogProxy);

    // 提交事务
    LONG error = DetourTransactionCommit();
    if (error != NO_ERROR) {
        std::wstringstream wss;
        wss << L"Failed to set hook: " << error;
        OutputDebugStringW(wss.str().c_str());
        FreeLibrary(hDll);
        return;
    }

    OutputDebugStringW(L"Successfully hooked WriteLog function");
}

// 工作线程：设置 hook
void WorkerThread() {
    //hook_load_xml();
    hook_write_log();
}

// DLL 入口点
BOOL APIENTRY DllMain(HMODULE hModule, DWORD ul_reason_for_call, LPVOID lpReserved) {
    switch (ul_reason_for_call) {
    case DLL_PROCESS_ATTACH: {
        // 启动工作线程
        HANDLE hThread = CreateThread(nullptr, 0, [](LPVOID) -> DWORD {
            WorkerThread();
            return 0;
            }, nullptr, 0, nullptr);
        if (hThread) {
            CloseHandle(hThread);
        }
        break;
    }
    case DLL_PROCESS_DETACH: {
        // 清理 hook
        if (OriginalLoadXML != nullptr) {
            DetourTransactionBegin();
            DetourUpdateThread(GetCurrentThread());
            DetourDetach(&(PVOID&)OriginalLoadXML, LoadXMLProxy);
            DetourTransactionCommit();
        }
        break;
    }
    case DLL_THREAD_ATTACH:
    case DLL_THREAD_DETACH:
        break;
    }
    return TRUE;
}