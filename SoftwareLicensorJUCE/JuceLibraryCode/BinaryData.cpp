/* ==================================== JUCER_BINARY_RESOURCE ====================================

   This is an auto-generated file: Any edits you make may be overwritten!

*/

#include <cstring>

namespace BinaryData
{

//================== Texts_en.properties ==================
static const unsigned char temp_binary_data_0[] =
"licenseActivated=Your license has been successfully activated.\n"
"licenseNotFound=No license found.\n"
"licenseMachineLimit=Your license has reached the machine limit.\n"
"trialEnded=Your trial has ended.\n"
"licenseInactive=Your license is no longer active.\n"
"offlineCodeIncorrect=Your offline code was incorrect.\n"
"offlineCodesDisabled=Offline codes are not enabled for this product.\n"
"licenseCodeInvalid=The license code was invalid.\n"
"machineDeactivated=This machine has been deactivated.";

const char* Texts_en_properties = (const char*) temp_binary_data_0;

//================== Texts_es.properties ==================
static const unsigned char temp_binary_data_1[] =
"licenseActivated=Su licencia ha sido activada exitosamente.\n"
"licenseNotFound=No se encontr\xc3\xb3 ninguna licencia.\n"
"licenseMachineLimit=Su licencia ha alcanzado el l\xc3\xadmite de m\xc3\xa1quinas.\n"
"trialEnded=Su prueba ha terminado.\n"
"licenseInactive=Su licencia ya no est\xc3\xa1 activa.\n"
"offlineCodeIncorrect=Su c\xc3\xb3""digo offline fue incorrecto.\n"
"offlineCodesDisabled=Los c\xc3\xb3""digos offline no est\xc3\xa1n habilitados para este producto.\n"
"licenseCodeInvalid=El c\xc3\xb3""digo de licencia no es v\xc3\xa1lido.\n"
"machineDeactivated=Esta m\xc3\xa1quina ha sido desactivada.";

const char* Texts_es_properties = (const char*) temp_binary_data_1;

//================== Texts_fr.properties ==================
static const unsigned char temp_binary_data_2[] =
"licenseActivated=Votre licence a \xc3\xa9t\xc3\xa9 activ\xc3\xa9""e avec succ\xc3\xa8s.\n"
"licenseNotFound=Aucune licence trouv\xc3\xa9""e.\n"
"licenseMachineLimit=Votre licence a atteint la limite de machines.\n"
"trialEnded=Votre p\xc3\xa9riode d'essai est termin\xc3\xa9""e.\n"
"licenseInactive=Votre licence n'est plus active.\n"
"offlineCodeIncorrect=Votre code hors ligne \xc3\xa9tait incorrect.\n"
"offlineCodesDisabled=Les codes hors ligne ne sont pas activ\xc3\xa9s pour ce produit.\n"
"licenseCodeInvalid=Le code de licence \xc3\xa9tait invalide.\n"
"machineDeactivated=Cette machine a \xc3\xa9t\xc3\xa9 d\xc3\xa9sactiv\xc3\xa9""e.";

const char* Texts_fr_properties = (const char*) temp_binary_data_2;


const char* getNamedResource (const char* resourceNameUTF8, int& numBytes);
const char* getNamedResource (const char* resourceNameUTF8, int& numBytes)
{
    unsigned int hash = 0;

    if (resourceNameUTF8 != nullptr)
        while (*resourceNameUTF8 != 0)
            hash = 31 * hash + (unsigned int) *resourceNameUTF8++;

    switch (hash)
    {
        case 0xafbff2d0:  numBytes = 469; return Texts_en_properties;
        case 0xd6382dab:  numBytes = 513; return Texts_es_properties;
        case 0xbd098ecd:  numBytes = 518; return Texts_fr_properties;
        default: break;
    }

    numBytes = 0;
    return nullptr;
}

const char* namedResourceList[] =
{
    "Texts_en_properties",
    "Texts_es_properties",
    "Texts_fr_properties"
};

const char* originalFilenames[] =
{
    "Texts_en.properties",
    "Texts_es.properties",
    "Texts_fr.properties"
};

const char* getNamedResourceOriginalFilename (const char* resourceNameUTF8);
const char* getNamedResourceOriginalFilename (const char* resourceNameUTF8)
{
    for (unsigned int i = 0; i < (sizeof (namedResourceList) / sizeof (namedResourceList[0])); ++i)
        if (strcmp (namedResourceList[i], resourceNameUTF8) == 0)
            return originalFilenames[i];

    return nullptr;
}

}
