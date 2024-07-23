#include <iostream>

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
    };

    /**
     * Checks the locally stored license data, and performs an API request if 
     * needed. Returns the data to the callback function.
     * 
     * Note that the callback function may not be able to be inlined 
     * properly, creating a weak point. However, this function will still 
     * overwrite the locally stored license data, and 
     * `check_license_no_api_request` can still return the information from 
     * the locally stored file.
     */
    void check_license(const char* company_name, const char* store_id, const char* machine_id, const char** product_ids_and_pubkeys, int len, void(*callback)(LicenseData*));

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

void process_license_data(LicenseData* data) {
    std::cout << "Received license data: " << std::endl;
    std::cout << "Result code: " << data->result_code << std::endl;
    std::cout << "First name: " << data->customer_first_name << std::endl;
    std::cout << "Last name: " << data->customer_last_name << std::endl;
    std::cout << "Email: " << data->customer_email << std::endl;
    std::cout << "License type: " << data->license_type << std::endl;
    std::cout << "Version: " << data->version << std::endl;
    std::cout << "Error message: " << data->error_message << std::endl;

    free_license_data(data);
}