#include <iostream>
#include "include/licensing.h"

int main() {
    std::cout << "Testing" << std::endl;

    const char* company_name = "SoftwareLicensorTestCompany";
    const char* product_ids_and_pubkeys[] = {
        "TestCq16-GntSNzpb4pMv1MTpNMygwvPI;BN68qc4GtF+cj0LZv/MPx+Hua/kIz1AgN3KKqu6PU2zU0OXdXEUhVj9FuhO7ScN0vXc5OoajFEA0sXj7/6wQVp/N6JmBleVGaE4oB4SlZ67sso9O7TgwT+db3xTKCj0/Bg==",
        "TestCq16-HlzFtdmTJfFMnBfLryBzbmpv;BE9LdpRxJYKzPMKBabVBG6hxQE0FPvg2mLXduzHNEhmSZX+ii1kJejgqMjoV4qq62GMnIngByPeP0cx++R5DQcoGTd3KQR7VDz7WnynhzYO3ecNlS4MLqtqeopm48/QNXg=="
    };
    int len = 2;
    const char* store_id = "TESTY3GK-ltAKyjzOicZ8a1WTGzQqQ2ra1c9ECsr8mFw4XcT_cPLOFfDMlGUZMYKF";
    const char* machine_id = "machine_id";

    const char* license_code = "E763-446A-7CF7-FD97-DFF5";


    auto data = read_reply_from_webserver(company_name, store_id, machine_id, license_code, product_ids_and_pubkeys, len);

    std::cout << "Received license data from webserver: " << std::endl;
    std::cout << "Result code: " << data->result_code << std::endl;
    std::cout << "First name: " << data->customer_first_name << std::endl;
    std::cout << "Last name: " << data->customer_last_name << std::endl;
    std::cout << "Email: " << data->customer_email << std::endl;
    std::cout << "License type: " << data->license_type << std::endl;
    std::cout << "Version: " << data->version << std::endl;
    std::cout << "Error message: " << data->error_message << std::endl;

    free_license_data(data);


    auto data_3 = check_license(company_name, store_id, machine_id, product_ids_and_pubkeys, len);

    free_license_data(data_3);
    auto data_2 = check_license_no_api_request(company_name, store_id, machine_id, product_ids_and_pubkeys, len);
    std::cout << "Loaded data from license file: " << std::endl;
    std::cout << "Result code: " << data_2->result_code << std::endl;
    std::cout << "First name: " << data_2->customer_first_name << std::endl;
    std::cout << "Last name: " << data_2->customer_last_name << std::endl;
    std::cout << "Email: " << data_2->customer_email << std::endl;
    std::cout << "License type: " << data_2->license_type << std::endl;
    std::cout << "Version: " << data_2->version << std::endl;
    std::cout << "Error message: " << data_2->error_message << std::endl;

    free_license_data(data_2);
    return 0;
}