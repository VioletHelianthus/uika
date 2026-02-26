// Helper to reconstruct FName from packed UikaFNameHandle.
// FName is 12 bytes in Editor builds (WITH_CASE_PRESERVING_NAME adds DisplayIndex),
// so reinterpret_cast<FName*>(&uint64) is UB and reads past the 8-byte handle.
#pragma once
#include "UObject/NameTypes.h"

static inline FName UikaUnpackFName(uint64_t Packed)
{
    FNameEntryId CompIdx = FNameEntryId::FromUnstableInt(static_cast<uint32>(Packed & 0xFFFFFFFF));
    int32 Number = static_cast<int32>(Packed >> 32);
    return FName(CompIdx, CompIdx, Number);
}

static inline uint64_t UikaPackFName(const FName& Name)
{
    return static_cast<uint64>(Name.GetComparisonIndex().ToUnstableInt())
         | (static_cast<uint64>(Name.GetNumber()) << 32);
}
