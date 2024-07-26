/*
  ==============================================================================

    SoftwareLicensorUnlockForm.h
    Created: 24 Jul 2024 6:16:43pm
    Author:  somed

  ==============================================================================
*/

#pragma once

#include "JuceHeader.h"
#include "SoftwareLicensorStatus.h"

/**
 * @brief This is pretty much a carbon copy of juce_OnlineUnlockForm.h, but it uses
 * SoftwareLicensorStatus rather than OnlineUnlockStatus because I've already 
 * implemented the cryptography and API requests in Rust, and it uses a different
 * protocol than JUCE.
 */
class SoftwareLicensorUnlockForm : public juce::Component,
    private juce::Button::Listener
{
public:
    SoftwareLicensorUnlockForm (SoftwareLicensorStatus&,
                                const juce::String& userInstructions,
                                bool hasCancelButton = true,
                                bool overlayHasCancelButton = false);
    ~SoftwareLicensorUnlockForm() override;

    virtual void dismiss();

    void paint(juce::Graphics&) override;
    void resized() override;
    void lookAndFeelChanged() override;

    juce::Label message;
    juce::TextEditor licenseCodeBox;
    juce::ToggleButton shareHardwareInfoButton{ "Share hardware information?" };
    juce::TextButton activateButton, cancelButton;

private:
    SoftwareLicensorStatus& status;
    std::unique_ptr<juce::BubbleMessageComponent> bubble;

    bool showOverlayCancelButton;

    struct OverlayComp;
    friend struct OverlayComp;
    juce::Component::SafePointer<juce::Component> unlockingOverlay;

    void buttonClicked(juce::Button*) override;
    void attemptRegistration();
    void showBubbleMessage(const juce::String&, juce::Component&);

    JUCE_DECLARE_NON_COPYABLE_WITH_LEAK_DETECTOR(SoftwareLicensorUnlockForm)
};