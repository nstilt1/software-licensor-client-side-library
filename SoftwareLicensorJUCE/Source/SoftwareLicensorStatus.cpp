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

    update_machine_info(
      should_update,
    );
}
#endif