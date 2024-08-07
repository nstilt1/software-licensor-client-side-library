/*
  ==============================================================================

    SoftwareLicensorMarketplaceStatus.h
    Created: 23 Jul 2024 3:46:53pm
    Author:  Noah Stiltner

  ==============================================================================
*/

#pragma once

#include "JuceHeader.h"

extern "C" {
#include <cstdint>

    /**
     * License data returned by the static Rust library. This must be freed
     * with the provided `free_license_data` function to prevent a memory leak.
     */
    struct LicenseData {
        int32_t result_code;
        char* customer_first_name;
        char* customer_last_name;
        char* customer_email;
        char* license_type;
        char* version;
        char* error_message;
        char* license_code;
    };

    /**
     * Checks the locally stored license data, and performs an API request if
     * needed.
     */
    LicenseData* check_license(const char* company_name, const char* store_id, const char* machine_id, const char** product_ids_and_pubkeys, int len);

    /**
     * Submits an API request to the Software Licensor serverless endpoint
     * to grab the latest license information. Adds a 5 second delay to the
     * response to deter brute force attacks.
     */
    LicenseData* read_reply_from_webserver(const char* company_name, const char* store_id, const char* machine_id, const char* license_code, const char** product_ids_and_pubkeys, int len);

    /**
     * Checks the license file with a guarantee that it will not ping the
     * server for an update. Keep in mind that almost all license types
     * have a built-in expiration, and this expiration needs to be renewed
     * via the `check_license` function's API call.
     *
     * This function is still asynchronous due to file system reads, but it
     * should be faster than `check_license` in some cases.
     */
    LicenseData* check_license_no_api_request(const char* company_name, const char* store_id, const char* machine_id, const char** product_ids_and_pubkeys, int len);

    /**
     * Frees the license data. This must be called for every instance of the
     * created license data.
     */
    void free_license_data(LicenseData* ptr);

    /**
     * Updates locally stored machine info that is sent to the server.
     *
     * `save_system_stats` should be the parameter that determines whether or
     * not to actually save all of these stats. There isn't a neat way to
     * supply all of these values to the Rust code, and not all of these values
     * can be determined within Rust for all machines, but they can be
     * determined using the JUCE library.
     */
    void update_machine_info(
        const char* company_name,
        bool save_system_stats,
        const char* os_name,
        const char* computer_name,
        bool is_64_bit,
        const char* users_language,
        const char* display_language,
        int num_logical_cores,
        int num_physical_cores,
        int cpu_freq_mhz,
        int ram_mb,
        int page_size,
        const char* cpu_vendor,
        const char* cpu_model,
        bool has_mmx,
        bool has_3d_now,
        bool has_fma3,
        bool has_fma4,
        bool has_sse,
        bool has_sse2,
        bool has_sse3,
        bool has_ssse3,
        bool has_sse41,
        bool has_sse42,
        bool has_avx,
        bool has_avx2,
        bool has_avx512f,
        bool has_avx512bw,
        bool has_avx512cd,
        bool has_avx512dq,
        bool has_avx512er,
        bool has_avx512ifma,
        bool has_avx512pf,
        bool has_avx512vbmi,
        bool has_avx512vl,
        bool has_avx512vpopcntdq,
        bool has_neon
    );
}

class SoftwareLicensorStatus
{
public:
    SoftwareLicensorStatus();

    /** Destructor. */
    virtual ~SoftwareLicensorStatus();

    /* Your store's ID provided by Software Licensor */
    virtual juce::String getStoreId() = 0;

    /**
    * This company name is primarily for storing all of your plugins' license files 
    * in the same place. It doesn't need to be a company name, but just a shared 
    * name that your plugins will use. This will store the license file in a directory
    * including that name. This may not work on iOS.
    * 
    * It would be best if there weren't any spaces in this string.
    */
    virtual juce::String getCompanyName() = 0;

    /**
    * Each string in this array of strings must be a product ID, followed by a 
    * semicolon, followed by the public key for the product ID.
    * 
    * This can take multiple pairs in the event that you're using a bundle and/or 
    * individual products.
    */
    virtual std::vector<juce::String> getProductIdsAndPubkeys() = 0;

    inline void authorizeLicenseCodeWithWebServer(juce::String licenseCode) {
        auto machine_id = juce::OnlineUnlockStatus::MachineIDUtilities::getUniqueMachineID();
        auto productIdsAndPubkeys = this->getProductIdsAndPubkeys();
        std::vector<const char*> product_cstrings;
        for (const auto& juceStr : productIdsAndPubkeys) {
            product_cstrings.push_back(juceStr.getCharPointer().getAddress());
        }

        auto companyName = this->getCompanyName();
        
        auto license_data = read_reply_from_webserver(
            this->getCompanyName().getCharPointer().getAddress(),
            this->getStoreId().getCharPointer().getAddress(),
            machine_id.getCharPointer().getAddress(),
            licenseCode.getCharPointer().getAddress(),
            product_cstrings.data(),
            product_cstrings.size()
        );

        process_license_data(license_data);
    }

    /**
     * @brief Checks the locally stored license file, and makes an API request if
     * needed. The API request might take about 200-600 ms depending on the server
     * load, with a higher load potentially resulting in faster times.
     * 
     * API requests need to be made occasionally to check on the license status. The
     * local license file can expire, but requests will renew the expiration.
     */
    inline juce::var check_license_with_potential_api_request() {
        auto machine_id = juce::OnlineUnlockStatus::MachineIDUtilities::getUniqueMachineID();
        auto productIdsAndPubkeys = this->getProductIdsAndPubkeys();

        std::vector<const char*> product_cstrings;
        for (const auto& juceStr : productIdsAndPubkeys) {
            product_cstrings.push_back(juceStr.getCharPointer().getAddress());
        }

        auto companyName = this->getCompanyName();

        auto license_data = check_license(
            this->getCompanyName().getCharPointer().getAddress(),
            this->getStoreId().getCharPointer().getAddress(),
            machine_id.getCharPointer().getAddress(),
            product_cstrings.data(),
            product_cstrings.size()
        );

        process_license_data(license_data);

        return isUnlocked();
    }

    /**
     * @brief Checks the locally stored license file without making a request to the
     * server.
     */
    inline juce::var check_license_with_no_api_request() {
        auto machine_id = juce::OnlineUnlockStatus::MachineIDUtilities::getUniqueMachineID();
        auto productIdsAndPubkeys = this->getProductIdsAndPubkeys();

        std::vector<const char*> product_cstrings;
        for (const auto& juceStr : productIdsAndPubkeys) {
            product_cstrings.push_back(juceStr.getCharPointer().getAddress());
        }

        auto license_data = check_license_no_api_request(
            this->getCompanyName().getCharPointer().getAddress(),
            this->getStoreId().getCharPointer().getAddress(),
            machine_id.getCharPointer().getAddress(),
            product_cstrings.data(),
            product_cstrings.size()
        );

        process_license_data(license_data);

        return isUnlocked();
    }

    /**
     * @brief Updates machine info that will be sent to the Service. If the bool is false,
     * the locally stored values is replaced with a `None` value, and will overwrite the 
     * server's stored value with `None` as well.
     * @param should_update 
     */
    void update_machine_information(bool should_update);

    /**
     * @brief Returns the license status. Values below 0 are errors such as file IO
     * errors or an API Error. Call getErrorMessage in that case. Other values 
     * include:
     *
     * 1: success
     * 2: no license found
     * 4: reached the machine limit
     * 8: trial ended
     * 16: license no longer active
     * 32: incorrect offline code (not currently enabled in our backend)
     * 64: offline codes are not allowed for this product
     * 128: invalid license code
     * 256: machine deactivated
     * 512: invalid license type
     * 
     * These values can be obtained with equals operations or bitwise and operations.
     * 
     * @return license status code
     */
    int getLicenseStatusCode() {
        return (int)status.getProperty(licenseStatusProp);
    }

    juce::String getUserFirstName() {
        return status.getProperty(firstNameProp);
    }

    juce::String getUserLastName() {
        return status.getProperty(lastNameProp);
    }

    juce::String getUserEmail() {
        return status.getProperty(emailProp);
    }

    juce::String getLicenseType() {
        return status.getProperty(licenseTypeProp);
    }

    juce::String getLicenseCode() {
        return status.getProperty(licenseCodeProp);
    }

    /**
     * @brief Gets the version of this software that the cloud has a record of.
     * 
     * If you do not update this value in the cloud, then this won't be of use to you.
     * You could alternatively achieve the same result by hosting a github page or an
     * S3 bucket that contains the current version, but this should work too.
     * @return the version of this software stored in the cloud
     */
    juce::String getCloudVersion() {
        return status.getProperty(versionProp);
    }

    /**
     * @brief This will return a message if there is one. This can be overriden
     * if you wish to use custom messages.
     * @return 
     */
    virtual juce::String getMessage() {
        if (getLicenseStatusCode() < 1)
            return status.getProperty(errorProp);

        juce::String enFileContents = R"(
language: English

"licenseActivated" = "Your license has been successfully activated."
"licenseNotFound" = "No license found."
"licenseMachineLimit" = "Your license has reached the machine limit."
"trialEnded" = "Your trial has ended."
"licenseInactive" = "Your license is no longer active."
"offlineCodeIncorrect" = "Your offline code was incorrect."
"offlineCodesDisabled" = "Offline codes are not enabled for this product."
"licenseCodeInvalid" = "The license code was invalid."
"machineDeactivated" = "This machine has been deactivated."
)";
        auto language = juce::SystemStats::getDisplayLanguage().substring(0,2);
        if (language == "en") 
        {
            std::unique_ptr<juce::LocalisedStrings> strings(new juce::LocalisedStrings(enFileContents, true));
            juce::LocalisedStrings::setCurrentMappings(strings.release());
        }
        else if (language == "fr") 
        {
            juce::String fileContents = R"(
language: French

"licenseActivated" = "Votre licence a �t� activ�e avec succ�s."
"licenseNotFound" = "Aucune licence trouv�e."
"licenseMachineLimit" = "Votre licence a atteint la limite de machines."
"trialEnded" = "Votre p�riode d'essai est termin�e."
"licenseInactive" = "Votre licence n'est plus active."
"offlineCodeIncorrect" = "Votre code hors ligne �tait incorrect."
"offlineCodesDisabled" = "Les codes hors ligne ne sont pas activ�s pour ce produit."
"licenseCodeInvalid" = "Le code de licence �tait invalide."
"machineDeactivated" = "Cette machine a �t� d�sactiv�e."
)";
            std::unique_ptr<juce::LocalisedStrings> strings(new juce::LocalisedStrings(fileContents, true));
            juce::LocalisedStrings::setCurrentMappings(strings.release());
        }
        else if (language == "es") 
        {
            juce::String fileContents = R"(
language: Spanish

"licenseActivated" = "Su licencia ha sido activada exitosamente."
"licenseNotFound" = "No se encontr� ninguna licencia."
"licenseMachineLimit" = "Su licencia ha alcanzado el l�mite de m�quinas."
"trialEnded" = "Su prueba ha terminado."
"licenseInactive" = "Su licencia ya no est� activa."
"offlineCodeIncorrect" = "Su c�digo offline fue incorrecto."
"offlineCodesDisabled" = "Los c�digos offline no est�n habilitados para este producto."
"licenseCodeInvalid" = "El c�digo de licencia no es v�lido."
"machineDeactivated" = "Esta m�quina ha sido desactivada."
)";
            std::unique_ptr<juce::LocalisedStrings> strings(new juce::LocalisedStrings(fileContents, true));
            juce::LocalisedStrings::setCurrentMappings(strings.release());
        }
        else 
        {
            std::unique_ptr<juce::LocalisedStrings> strings(new juce::LocalisedStrings(enFileContents, true));
            juce::LocalisedStrings::setCurrentMappings(strings.release());
        }
        
        switch (getLicenseStatusCode()) {
            case 1: return juce::translate("licenseActivated");
            case 2: return juce::translate("licenseNotFound");
            case 4: return juce::translate("licenseMachineLimit");
            case 8: return juce::translate("trialEnded");
            case 16: return juce::translate("licenseInactive");
            case 32: return juce::translate("offlineCodeIncorrect");
            case 64: return juce::translate("offlineCodesDisabled");
            case 128: return juce::translate("licenseCodeInvalid");
            case 256: return juce::translate("machineDeactivated");
            default: return juce::translate("Unknown error");
        }
    }

    inline juce::var isUnlocked() const { return (int)status[licenseStatusProp] == 1; }
private:
    juce::ValueTree status;

    /* Processes license data */
    inline void process_license_data(LicenseData* data) {
        // codes above 0 are licensing related, codes below 0 are 
        if (data->result_code > 0) {
            status.setProperty(licenseStatusProp, data->result_code, nullptr);
            status.setProperty(firstNameProp, data->customer_first_name, nullptr);
            status.setProperty(lastNameProp, data->customer_last_name, nullptr);
            status.setProperty(emailProp, data->customer_email, nullptr);
            status.setProperty(licenseTypeProp, data->license_type, nullptr);
            status.setProperty(versionProp, data->version, nullptr);
            status.setProperty(errorProp, data->error_message, nullptr);
            status.setProperty(licenseCodeProp, data->license_code, nullptr);
        } else {
            status.setProperty(firstNameProp, data->customer_first_name, nullptr);
            status.setProperty(lastNameProp, data->customer_last_name, nullptr);
            status.setProperty(emailProp, data->customer_email, nullptr);
            status.setProperty(licenseTypeProp, data->license_type, nullptr);
            status.setProperty(versionProp, data->version, nullptr);
            status.setProperty(errorProp, data->error_message, nullptr);
            status.setProperty(licenseCodeProp, "", nullptr);
        }
        free_license_data(data);
    }

    static const char* licenseStatusProp;
    static const char* firstNameProp;
    static const char* lastNameProp;
    static const char* emailProp;
    static const char* licenseTypeProp;
    static const char* versionProp;
    static const char* errorProp;
    static const char* licenseCodeProp;

    JUCE_DECLARE_NON_COPYABLE(SoftwareLicensorStatus)
};