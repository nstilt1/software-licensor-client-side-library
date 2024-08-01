/*
  ==============================================================================

    SoftwareLicensorUnlockForm.h
    Created: 24 Jul 2024 6:16:43pm
    Author:  Noah Stiltner

  ==============================================================================
*/

#pragma once

#include "SoftwareLicensorStatus.h"

/**
 * @brief This is pretty much a carbon copy of juce_OnlineUnlockForm.h, but it uses
 * SoftwareLicensorStatus rather than OnlineUnlockStatus because I've already 
 * implemented the cryptography and API requests in Rust, and it uses a different
 * protocol than JUCE. Since it is similar, I will also include the documentation 
 * for the OnlineUnlockForm
 */
/** Acts as a GUI which asks the user for their details, and calls the appropriate
    methods on your OnlineUnlockStatus object to attempt to register the app.

    You should create one of these components and add it to your parent window,
    or use a DialogWindow to display it as a pop-up. But if you're writing a plugin,
    then DO NOT USE A DIALOG WINDOW! Add it as a child component of your plugin's editor
    component instead. Plugins that pop up external registration windows are incredibly
    annoying, and cause all sorts of headaches for hosts. Don't be the person who
    writes that plugin that irritates everyone with a dialog box every time they
    try to scan for new plugins!

    Note that after adding it, you should put the component into a modal state,
    and it will automatically delete itself when it has completed.

    Although it deletes itself, it's also OK to delete it manually yourself
    if you need to get rid of it sooner.

    @see OnlineUnlockStatus

    @tags{ProductUnlocking}
*/
class SoftwareLicensorUnlockForm : public juce::Component,
    private juce::Button::Listener
{
public:
    SoftwareLicensorUnlockForm (SoftwareLicensorStatus&,
                                const juce::String& userInstructions,
                                bool hasCancelButton = true);
    ~SoftwareLicensorUnlockForm() override;

    /**
     * @brief An overridable dismiss function. Consider using setVisible(false) 
     * and exitModalState()
     */
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

    struct OverlayComp;
    friend struct OverlayComp;
    juce::Component::SafePointer<juce::Component> unlockingOverlay;

    void buttonClicked(juce::Button*) override;
    void attemptRegistration();
    void showBubbleMessage(const juce::String&, juce::Component&);

    JUCE_DECLARE_NON_COPYABLE_WITH_LEAK_DETECTOR(SoftwareLicensorUnlockForm)
};