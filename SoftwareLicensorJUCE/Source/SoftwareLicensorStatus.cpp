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
const char* SoftwareLicensorStatus::licenseCodeProp = "licenseCode";
static const char* stateTagName = "REG";

SoftwareLicensorStatus::SoftwareLicensorStatus() : status(stateTagName)
{
}

SoftwareLicensorStatus::~SoftwareLicensorStatus()
{
}

void SoftwareLicensorStatus::update_machine_information(bool should_update) {
    /**
     * If your code breaks here, perhaps with an error like "*this* is 0xFFFFFFFFFF",
     * ensure that the unlockForm member of your PluginEditor is being correctly initialized 
     * in the constructor of the PluginEditor, particularly in the initializer list where it 
     * might say:
     * MyProcessorEditor::MyProcessorEditor(MyAudioProcessor& p)
     *  : AudioProcessorEditor(&p), audioProcessor(p), unlockForm(audioProcessor.unlockStatus)
     * 
     * If it specifically says `audioProcessor.unlockStatus`, you may get this error. Instead 
     * use unlockForm(p.unlockStatus), or whatever variable name you have for the MyAudioProcessor&
     */
    auto osName = juce::SystemStats::getOperatingSystemName().toStdString();
    auto computerName = juce::SystemStats::getComputerName().toStdString();
    auto userLanguage = juce::SystemStats::getUserLanguage().toStdString();
    auto displayLanguage = juce::SystemStats::getDisplayLanguage().toStdString();
    auto cpuVendor = juce::SystemStats::getCpuVendor().toStdString();
    auto cpuModel = juce::SystemStats::getCpuModel().toStdString();

    update_machine_info(
        should_update,
        osName.c_str(),
        computerName.c_str(),
        juce::SystemStats::isOperatingSystem64Bit(),
        userLanguage.c_str(),
        displayLanguage.c_str(),
        juce::SystemStats::getNumCpus(),
        juce::SystemStats::getNumPhysicalCpus(),
        juce::SystemStats::getCpuSpeedInMegahertz(),
        juce::SystemStats::getMemorySizeInMegabytes(),
        juce::SystemStats::getPageSize(),
        cpuVendor.c_str(),
        cpuModel.c_str(),
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