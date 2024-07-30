/*
  ==============================================================================

    SoftwareLicensorStatus.cpp
    Created: 24 Jul 2024 6:04:50pm
    Author:  Noah Stiltner

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

void SoftwareLicensorStatus::update_machine_information(bool should_update) {
    juce::String companyName = this->getCompanyName();
    auto companyNamePtr = companyName.getCharPointer();
    auto osName = juce::SystemStats::getOperatingSystemName().getCharPointer();
    auto computerName = juce::SystemStats::getComputerName().getCharPointer();
    auto userLanguage = juce::SystemStats::getUserLanguage().getCharPointer();
    auto displayLanguage = juce::SystemStats::getDisplayLanguage().getCharPointer();
    auto cpuVendor = juce::SystemStats::getCpuVendor().getCharPointer();
    auto cpuModel = juce::SystemStats::getCpuModel().getCharPointer();

    update_machine_info(
        companyNamePtr.getAddress(),
        should_update,
        osName.getAddress(),
        computerName.getAddress(),
        juce::SystemStats::isOperatingSystem64Bit(),
        userLanguage.getAddress(),
        displayLanguage.getAddress(),
        juce::SystemStats::getNumCpus(),
        juce::SystemStats::getNumPhysicalCpus(),
        juce::SystemStats::getCpuSpeedInMegahertz(),
        juce::SystemStats::getMemorySizeInMegabytes(),
        juce::SystemStats::getPageSize(),
        cpuVendor.getAddress(),
        cpuModel.getAddress(),
        juce::SystemStats::hasMMX(),
        juce::SystemStats::has3DNow(),
        juce::SystemStats::hasFMA3(),
        juce::SystemStats::hasFMA4(),
        juce::SystemStats::hasSSE(),
        juce::SystemStats::hasSSE2(),
        juce::SystemStats::hasSSE3(),
        juce::SystemStats::hasSSSE3(),
        juce::SystemStats::hasSSE41(),
        juce::SystemStats::hasSSE42(),
        juce::SystemStats::hasAVX(),
        juce::SystemStats::hasAVX2(),
        juce::SystemStats::hasAVX512F(),
        juce::SystemStats::hasAVX512BW(),
        juce::SystemStats::hasAVX512CD(),
        juce::SystemStats::hasAVX512DQ(),
        juce::SystemStats::hasAVX512ER(),
        juce::SystemStats::hasAVX512IFMA(),
        juce::SystemStats::hasAVX512PF(),
        juce::SystemStats::hasAVX512VBMI(),
        juce::SystemStats::hasAVX512VL(),
        juce::SystemStats::hasAVX512VPOPCNTDQ(),
        juce::SystemStats::hasNeon()
    );
}


#endif