//
// Copyright (C) Microsoft Corporation. All rights reserved.
//
#include "pch.h"
#include "../gate0/TestPattern.h"
#ifdef GATE0_USE_PRODUCER
#include <vector>
#include "../gate0/SharedFrame.h"
#include "../gate0/SharedSection.h"
#ifndef GATE0_SECTION_NAME
#define GATE0_SECTION_NAME L"Global\\gate0-vcam-frames"
// "Global\gate0-vcam-frames" is 24 characters plus the terminator; catches a lost backslash escape.
static_assert(sizeof(GATE0_SECTION_NAME) == 25 * sizeof(wchar_t), "GATE0_SECTION_NAME escape error");
#endif
#ifndef GATE0_DLL_CREATES
#define GATE0_DLL_CREATES false     // experiment A: producer creates the section; B: build with /DGATE0_DLL_CREATES=true
#endif
#endif

HRESULT SimpleFrameGenerator::Initialize(_In_ IMFMediaType* pMediaType)
{
    RETURN_HR_IF_NULL(E_INVALIDARG, pMediaType);

    RETURN_IF_FAILED(pMediaType->GetGUID(MF_MT_SUBTYPE, &m_subType));
    if (m_subType != MFVideoFormat_RGB32 && m_subType != MFVideoFormat_NV12)
    {
        RETURN_HR_MSG(MF_E_UNSUPPORTED_FORMAT, "Unsupported format: %s", winrt::to_hstring(m_subType).data());
    }
    MFGetAttributeSize(pMediaType, MF_MT_FRAME_SIZE, &m_width, &m_height);
    m_frameIndex = 0;

    return S_OK;
}

/*:
   Writes to a buffer representing a 2D image.
   Writes a different constant to each line based on row number and current time.
   Assumes top down image, no negative stride and pBuf points to the begnning of the buffer of length len.
   Param:
   pBuf - pointer to beginning of buffer
   pitch - line length in bytes
   len - length of buffer in bytes
*/
HRESULT SimpleFrameGenerator::CreateFrame(
    _Inout_updates_bytes_(len) BYTE* pBuf,
    _In_ DWORD len,
    _In_ LONG pitch,
    _In_ ULONG rgbMask)
{
    if (m_subType == MFVideoFormat_RGB32)
    {
        DEBUG_MSG(L"RGB32 frames %s\n", winrt::to_hstring(MFVideoFormat_RGB32).data());

        RETURN_IF_FAILED(_CreateRGB32Frame(pBuf, len, pitch, m_width, m_height, rgbMask));
    }
    else if(m_subType == MFVideoFormat_NV12)
    {
        DEBUG_MSG(L"NV12 frames %s \n", winrt::to_hstring(MFVideoFormat_NV12).data());

        DWORD frameBuffLen = m_width * m_height * 4;
        wil::unique_cotaskmem_ptr<BYTE[]> spBuff = wil::make_unique_cotaskmem_nothrow<BYTE[]>(frameBuffLen);
        RETURN_IF_NULL_ALLOC(spBuff.get());

        RETURN_IF_FAILED(_CreateRGB32Frame(spBuff.get(), frameBuffLen, m_width * 4, m_width, m_height, rgbMask));
        RETURN_IF_FAILED(RGB32ToNV12Frame(spBuff.get(), frameBuffLen, m_width * 4, m_width, m_height, pBuf, len, pitch));
    }
    else
    {
        return MF_E_UNSUPPORTED_FORMAT;
    }

    ++m_frameIndex;
    return S_OK;
}

//////////////////////////////////////////////////
// private

HRESULT SimpleFrameGenerator::_CreateRGB32Frame(
    _Inout_updates_bytes_(len) BYTE* pBuf,
    _In_ DWORD len,
    _In_ LONG pitch,
    _In_ DWORD width,
    _In_ DWORD height,
    _In_ ULONG rgbMask )
{
    RETURN_HR_IF_NULL(E_INVALIDARG, pBuf);
    if (len < (abs(pitch) * height ))
    {
        return HRESULT_FROM_WIN32(ERROR_INSUFFICIENT_BUFFER);
    }

#ifdef GATE0_USE_PRODUCER
    {
        // Producer mode: copy the producer's 1280x720 frame (scaled to the negotiated size) or show a flat slate. Never a stale frame.
        constexpr uint32_t kSrcW = 1280, kSrcH = 720;
        constexpr size_t kMapBytes = gate0::kHeaderBytes + static_cast<size_t>(kSrcW) * kSrcH * 4;
        static gate0::SharedSection s_section;
        static std::vector<uint8_t> s_src(static_cast<size_t>(kSrcW) * kSrcH * 4);
        uint64_t producerCounter = 0;
        gate0::ReadStatus st = gate0::ReadStatus::BadHeader;
        if (s_section.EnsureOpen(GATE0_SECTION_NAME, kMapBytes, GATE0_DLL_CREATES))
        {
            for (int attempt = 0; attempt < 3; ++attempt)
            {
                st = gate0::TryReadFrame(s_section.base(), s_section.size(), GetTickCount64(), kSrcW, kSrcH, s_src.data(), kSrcW * 4, &producerCounter);
                if (st != gate0::ReadStatus::Torn) break;
            }
        }
        if (st == gate0::ReadStatus::Ok)
        {
            for (DWORD y = 0; y < height; ++y)
            {
                const uint32_t* srcRow = reinterpret_cast<const uint32_t*>(s_src.data()) + static_cast<size_t>(y * kSrcH / height) * kSrcW;
                uint32_t* dstRow = reinterpret_cast<uint32_t*>(pBuf + static_cast<size_t>(y) * pitch);
                for (DWORD x = 0; x < width; ++x) dstRow[x] = srcRow[x * kSrcW / width];
            }
            gate0::DrawCounterStrip(pBuf, static_cast<int>(width), static_cast<int>(height), static_cast<int>(pitch),
                                    static_cast<uint32_t>(producerCounter), gate0::kMarkerLive);
        }
        else
        {
            gate0::RenderTestPattern(pBuf, static_cast<int>(width), static_cast<int>(height), static_cast<int>(pitch), m_frameIndex, gate0::kMarkerSlate);
        }
    }
#else
    // GATE0: moving test pattern + counter strip (see gate0/TestPattern.h) replaces the sample's grey gradient.
    gate0::RenderTestPattern(reinterpret_cast<uint8_t*>(pBuf), static_cast<int>(width), static_cast<int>(height), static_cast<int>(pitch), m_frameIndex, gate0::kMarkerLive);

#endif

    return S_OK;
}

//////////////////////////////////////////////////
// pixelFormatConverter

void SimpleFrameGenerator::RGB24ToYUY2(int R, int G, int B, BYTE* pY, BYTE* pU, BYTE* pV)
{
    *pY = ((66 * R + 129 * G + 25 * B + 128) >> 8) + 16;
    *pU = ((-38 * R - 74 * G + 112 * B + 128) >> 8) + 128;
    *pV = ((112 * R - 94 * G - 18 * B + 128) >> 8) + 128;
}

void SimpleFrameGenerator::RGB24ToY(int R, int G, int B, BYTE* pY)
{
    *pY = ((66 * R + 129 * G + 25 * B + 128) >> 8) + 16;
}

void SimpleFrameGenerator::RGB32ToNV12(BYTE RGB1[8], BYTE RGB2[8], BYTE* pY1, BYTE* pY2, BYTE* pUV)
{
    RGB24ToYUY2(RGB1[2], RGB1[1], RGB1[0], pY1, pUV, pUV + 1);
    RGB24ToY(RGB1[6], RGB1[5], RGB1[4], pY1 + 1);
    RGB24ToYUY2(RGB2[2], RGB2[1], RGB2[0], pY2, pUV, pUV + 1);
    RGB24ToY(RGB2[6], RGB2[5], RGB2[4], pY2 + 1);
};

//////////////////////////////////////////////////
// FrameFormatConverter

HRESULT SimpleFrameGenerator::RGB32ToNV12Frame(_Inout_updates_bytes_(len) BYTE* pbBuff, ULONG cbBuff, long stride, UINT width, UINT height, BYTE* pbBuffOut, ULONG cbBuffOut, long strideOut)
{
    do
    {
        RETURN_HR_IF(E_UNEXPECTED, width * 4 * height > cbBuff);
        RETURN_HR_IF(E_UNEXPECTED, width * 1.5 * height > cbBuffOut);
        RETURN_HR_IF_NULL(E_INVALIDARG, pbBuff);

        RETURN_HR_IF_NULL(E_INVALIDARG, pbBuffOut);
        for (DWORD h = 0; h < height - 1; h += 2)
        {
            BYTE* pRGB1 = h * stride + pbBuff;
            BYTE* pRGB2 = (h + 1) * stride + pbBuff;
            BYTE* pY1 = h * strideOut + pbBuffOut;
            BYTE* pY2 = (h + 1) * strideOut + pbBuffOut;
            BYTE* pUV = (h / 2 + height) * strideOut + pbBuffOut;

            for (DWORD w = 0; w < width; w += 2)
            {
                RGB32ToNV12(pRGB1, pRGB2, pY1, pY2, pUV);
                pRGB1 += 8;
                pRGB2 += 8;
                pY1 += 2;
                pY2 += 2;
                pUV += 2;
            }
        }
    } while (FALSE);

    return S_OK;
}
