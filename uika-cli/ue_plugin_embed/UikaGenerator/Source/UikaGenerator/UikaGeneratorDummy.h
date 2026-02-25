// Dummy UCLASS to ensure UikaGenerator appears in the UHT manifest.
// The UikaExporter UBT plugin requires its ModuleName to be present in the manifest
// so that MakePath() can determine the output directory.

#pragma once

#include "CoreMinimal.h"
#include "UObject/Object.h"
#include "UikaGeneratorDummy.generated.h"

UCLASS(NotBlueprintable, Hidden)
class UUikaGeneratorDummy : public UObject
{
	GENERATED_BODY()
};
