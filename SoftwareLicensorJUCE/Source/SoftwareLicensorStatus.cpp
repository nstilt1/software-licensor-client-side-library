/*
  ==============================================================================

    SoftwareLicensorStatus.cpp
    Created: 24 Jul 2024 6:04:50pm
    Author:  somed

  ==============================================================================
*/

#include "SoftwareLicensorStatus.h"

#if JUCE_MODULE_AVAILABLE_juce_data_structures
const char* SoftwareLicensorStatus::licenseStatusProp = "L";
const char* SoftwareLicensorStatus::firstNameProp = "first";
const char* SoftwareLicensorStatus::lastNameProp = "last";
const char* SoftwareLicensorStatus::emailProp = "email";
const char* SoftwareLicensorStatus::licenseTypeProp = "licenseType";
const char* SoftwareLicensorStatus::versionProp = "version";
const char* SoftwareLicensorStatus::errorProp = "error";
static const char* stateTagName = "REG";

SoftwareLicensorStatus::SoftwareLicensorStatus() : status(stateTagName)
{
}

SoftwareLicensorStatus::~SoftwareLicensorStatus()
{
}


#endif