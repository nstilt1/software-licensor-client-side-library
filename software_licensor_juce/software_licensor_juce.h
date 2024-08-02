/*******************************************************************************
 BEGIN_JUCE_MODULE_DECLARATION

  ID:                 software_licensor_product_unlocking
  vendor:             software_licensor
  version:            1.0.0
  name:               Software Licensor Marketplace Support
  description:        Classes for online product authentication
  website:            https://github.com/nstilt1/software-licensor-client-side-library
  license:            AGPLv3/Commercial
  minimumCppStandard: 17
  OSXLibs:            libsoftwarelicensor.a
  windowsLibs:        softwarelicensor Userenv Ntdll Bcrypt Ws2_32

  dependencies:       juce_product_unlocking

 END_JUCE_MODULE_DECLARATION

*******************************************************************************/

#pragma once

#include <juce_core/juce_core.h>
#include <juce_cryptography/juce_cryptography.h>
#include <juce_data_structures/juce_data_structures.h>
#include <juce_events/juce_events.h>
#include <juce_graphics/juce_graphics.h>
#include <juce_gui_basics/juce_gui_basics.h>
#include <juce_gui_extra/juce_gui_extra.h>
#include <juce_product_unlocking/juce_product_unlocking.h>

#include "marketplace/SoftwareLicensorStatus.h"
#include "marketplace/SoftwareLicensorUnlockForm.h"