/*
  ==============================================================================

    SoftwareLicensorUnlockForm.cpp
    Created: 24 Jul 2024 6:16:43pm
    Author:  Noah Stiltner

  ==============================================================================
*/

#include "SoftwareLicensorUnlockForm.h"

struct Spinner : public juce::Component,
    private juce::Timer
{
    Spinner() { startTimer(1000 / 50); }
    void timerCallback() override { repaint(); }
    
    void paint(juce::Graphics& g) override
    {
        getLookAndFeel().drawSpinningWaitAnimation(g, juce::Colours::darkgrey, 0, 0, getWidth(), getHeight());
    }
};

struct SoftwareLicensorUnlockForm::OverlayComp : public juce::Component,
    private juce::Thread,
    private juce::Timer,
    private juce::Button::Listener
{
    OverlayComp (SoftwareLicensorUnlockForm& f)
        : juce::Thread(juce::String()), form(f)
    {
        licenseCode = form.licenseCodeBox.getText();
        addAndMakeVisible(spinner);

        startThread(Priority::normal);
    }

    ~OverlayComp() override
    {
        stopThread(10000);
    }

    void paint(juce::Graphics& g) override
    {
        g.fillAll(juce::Colours::white.withAlpha(0.97f));

        g.setColour(juce::Colours::black);
        g.setFont(15.0f);

        g.drawFittedText(TRANS("Contacting XYZ...").replace("XYZ", "server"),
            getLocalBounds().reduced(20, 0).removeFromTop(proportionOfHeight(0.6f)),
            juce::Justification::centred, 5);
    }

    void resized() override
    {
        const int spinnerSize = 40;
        spinner.setBounds((getWidth() - spinnerSize) / 2, proportionOfHeight(0.6f), spinnerSize, spinnerSize);

        if (cancelButton != nullptr)
            cancelButton->setBounds(getLocalBounds().removeFromBottom(50).reduced(getWidth() / 4, 5));
    }

    void run() override
    {
        form.status.authorizeLicenseCodeWithWebServer(licenseCode);
        startTimer(100);
    }

    void timerCallback() override
    {
        spinner.setVisible(false);
        stopTimer();

        auto statusCode = form.status.getLicenseStatusCode();

        if (statusCode != 1) 
        {
            juce::AlertWindow::showMessageBoxAsync(juce::MessageBoxIconType::WarningIcon,
                TRANS("Registration Failed"),
                form.status.getMessage());
        }
        else 
        {
            juce::AlertWindow::showMessageBoxAsync(juce::MessageBoxIconType::InfoIcon,
                TRANS("Registration Complete!"),
                form.status.getMessage());
        }

        const bool worked = form.status.isUnlocked();
        SoftwareLicensorUnlockForm& f = form;

        delete this;

        if (worked)
            f.dismiss();
    }

    void buttonClicked(juce::Button* button) override
    {
        if (button == cancelButton.get())
        {
            
        }
    }

    SoftwareLicensorUnlockForm& form;
    Spinner spinner;
    juce::String licenseCode;

    std::unique_ptr<juce::TextButton> cancelButton;

    JUCE_LEAK_DETECTOR (SoftwareLicensorUnlockForm::OverlayComp)
};

SoftwareLicensorUnlockForm::SoftwareLicensorUnlockForm(SoftwareLicensorStatus& s,
    const juce::String& userInstructions,
    bool hasCancelButton)
    : message(juce::String(), userInstructions),
    licenseCodeBox(juce::String()),
    activateButton(TRANS("Register")),
    cancelButton(TRANS("Cancel")),
    status(s)
{
    // supply a message to tell your users what to do
    jassert(userInstructions.isNotEmpty());

    setOpaque(true);

    licenseCodeBox.setText("");
    message.setJustificationType(juce::Justification::centred);

    addAndMakeVisible(message);
    addAndMakeVisible(licenseCodeBox);
    addAndMakeVisible(shareHardwareInfoButton);
    
    if (hasCancelButton)
        addAndMakeVisible(cancelButton);

    licenseCodeBox.setEscapeAndReturnKeysConsumed(false);

    addAndMakeVisible(activateButton);
    activateButton.addShortcut(juce::KeyPress(juce::KeyPress::returnKey));

    activateButton.addListener(this);
    cancelButton.addListener(this);

    lookAndFeelChanged();
    setSize(500, 250);
}

SoftwareLicensorUnlockForm::~SoftwareLicensorUnlockForm()
{
    unlockingOverlay.deleteAndZero();
}

void SoftwareLicensorUnlockForm::paint(juce::Graphics& g)
{
    g.fillAll(juce::Colours::darkslategrey);
}

void SoftwareLicensorUnlockForm::resized()
{
    /* If you're writing a plugin, then DO NOT USE A POP-UP A DIALOG WINDOW!
       Plugins that create external windows are incredibly annoying for users, and
       cause all sorts of headaches for hosts. Don't be the person who writes that
       plugin that irritates everyone with a nagging dialog box every time they scan!
    */
    jassert(juce::JUCEApplicationBase::isStandaloneApp() || findParentComponentOfClass<juce::DialogWindow>() == nullptr);

    const int buttonHeight = 22;

    auto r = getLocalBounds().reduced(10, 20);

    auto buttonArea = r.removeFromBottom(buttonHeight);
    activateButton.changeWidthToFitText(buttonHeight);
    cancelButton.changeWidthToFitText(buttonHeight);
    shareHardwareInfoButton.changeWidthToFitText();

    const int gap = 20;
    buttonArea = buttonArea.withSizeKeepingCentre(activateButton.getWidth()
        + (cancelButton.isVisible() ? gap + cancelButton.getWidth() : 0),
        buttonHeight);
    activateButton.setBounds(buttonArea.removeFromLeft(activateButton.getWidth()));
    buttonArea.removeFromLeft(gap);
    cancelButton.setBounds(buttonArea);


    r.removeFromBottom(20);

    juce::Font font(juce::Font::getDefaultTypefaceForFont(juce::Font(juce::Font::getDefaultSansSerifFontName(),
        juce::Font::getDefaultStyle(),
        5.0f)));

    const int boxHeight = 24;
    licenseCodeBox.setBounds(r.removeFromBottom(boxHeight));
    licenseCodeBox.setInputRestrictions(35, juce::String("abcdefABCDEF1234567890-olinOLIN"));
    licenseCodeBox.setFont(font);

    r.removeFromBottom(24);
    shareHardwareInfoButton.setBounds(r.removeFromBottom(24));

    message.setBounds(r.removeFromBottom(24));

    if (unlockingOverlay != nullptr)
        unlockingOverlay->setBounds(getLocalBounds());
}

void SoftwareLicensorUnlockForm::lookAndFeelChanged()
{
    juce::Colour labelCol(findColour(juce::TextEditor::backgroundColourId).contrasting(0.5f));

    licenseCodeBox.setTextToShowWhenEmpty(TRANS("License Code"), labelCol);
}

void SoftwareLicensorUnlockForm::showBubbleMessage(const juce::String& text, juce::Component& target)
{
    bubble.reset(new juce::BubbleMessageComponent(500));
    addChildComponent(bubble.get());

    juce::AttributedString attString;
    attString.append(text, juce::Font(16.0f));

    bubble->showAt(getLocalArea(&target, target.getLocalBounds()),
        attString, 500, // numMillisecondsBeforeRemoving
        true, // removeWhenMouseClicked
        false); // deleteSelfAfterUse
}

void SoftwareLicensorUnlockForm::buttonClicked(juce::Button* b)
{
    if (b == &activateButton)
    {
        attemptRegistration();
    }
    else if (b == &cancelButton)
    {
        dismiss();
    }
}

void SoftwareLicensorUnlockForm::attemptRegistration()
{
    if (unlockingOverlay == nullptr)
    {
        if (licenseCodeBox.getText().trim().length() < 16)
        {
            showBubbleMessage(TRANS("Please enter a valid license code!"), licenseCodeBox);
            return;
        }

        bool shareHardwareInfo = shareHardwareInfoButton.getToggleState();

        status.update_machine_information(shareHardwareInfo);

        addAndMakeVisible(unlockingOverlay = new OverlayComp(*this));
        resized();
        unlockingOverlay->enterModalState();
    }
}

void SoftwareLicensorUnlockForm::dismiss()
{
    delete this;
}