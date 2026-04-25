#include <iostream>
#include <fstream>
#include <sstream>
#include <string>
#include <vector>
#include <map>
#include <set>
#include <algorithm>

// Structure to represent a Device and its Schemes
struct Device {
    std::string alias;
    int scheme_count;
    // Each element in 'schemes' is a list of lines representing one scheme (child node)
    std::vector<std::vector<std::string>> schemes; 

    // Equality operator for comparison
    bool operator==(const Device& other) const {
        if (alias != other.alias) return false;
        if (scheme_count != other.scheme_count) return false;
        if (schemes.size() != other.schemes.size()) return false;
        
        // Schemes might be in different order in different files?
        // Assuming order matters for now or simple comparison. 
        // If the DTS parsing order is deterministic (which it is), we can compare directly.
        for (size_t i = 0; i < schemes.size(); ++i) {
            if (schemes[i].size() != other.schemes[i].size()) return false;
            for (size_t j = 0; j < schemes[i].size(); ++j) {
                if (schemes[i][j] != other.schemes[i][j]) return false;
            }
        }
        return true;
    }
};

// Function to parse a pinctrl file
// Returns a map where key is device alias, and value is Device object
std::map<std::string, Device> parse_file(const std::string& filename) {
    std::map<std::string, Device> devices;
    std::ifstream infile(filename);
    if (!infile.is_open()) {
        std::cerr << "Error: Could not open file " << filename << std::endl;
        exit(1);
    }

    std::string line;
    while (std::getline(infile, line)) {
        if (line.empty()) continue;

        // Parse device header: alias,count
        // e.g. "uart0,1"
        std::stringstream ss(line);
        std::string alias, count_str;
        if (std::getline(ss, alias, ',') && std::getline(ss, count_str)) {
            int count = std::stoi(count_str);
            Device dev;
            dev.alias = alias;
            dev.scheme_count = count;
            
            // Read schemes
            // Each scheme starts with "alias,pin_count"
            for (int i = 0; i < count; ++i) {
                std::vector<std::string> scheme_lines;
                if (std::getline(infile, line)) {
                    scheme_lines.push_back(line); // scheme header: "uart0-xfer,1"
                    
                    std::stringstream ss2(line);
                    std::string scheme_name, pin_count_str;
                    if (std::getline(ss2, scheme_name, ',') && std::getline(ss2, pin_count_str)) {
                         int pin_count = std::stoi(pin_count_str);
                         for (int p = 0; p < pin_count; ++p) {
                             if (std::getline(infile, line)) {
                                 scheme_lines.push_back(line);
                             }
                         }
                    }
                }
                dev.schemes.push_back(scheme_lines);
            }
            devices[alias] = dev;
        }
    }
    return devices;
}

int main(int argc, char* argv[]) {
    // Expected args: ./compare_pinctrl in1 ... inn out_common out1 ... outn
    // Total args (argc) = 1 + n + 1 + n = 2n + 2
    // So argc - 2 must be even (2n)
    
    if (argc < 4 || (argc - 2) % 2 != 0) {
        std::cerr << "Usage: " << argv[0] << " input_1 ... input_n output_common output_1 ... output_n" << std::endl;
        std::cerr << "Error: Incorrect number of arguments. Must provide n input files, 1 common output file, and n unique output files." << std::endl;
        return 1;
    }

    int n = (argc - 2) / 2;
    
    std::vector<std::string> input_files;
    for (int i = 1; i <= n; ++i) {
        input_files.push_back(argv[i]);
    }
    
    std::string common_output_file = argv[n + 1];
    
    std::vector<std::string> unique_output_files;
    for (int i = n + 2; i < argc; ++i) {
        unique_output_files.push_back(argv[i]);
    }

    // Parse all files
    std::vector<std::map<std::string, Device>> all_files_devices;
    for (const auto& filename : input_files) {
        all_files_devices.push_back(parse_file(filename));
    }

    // Helper lambda to write a device to stream
    auto write_device = [](std::ofstream& out, const Device& dev) {
        out << dev.alias << "," << dev.scheme_count << std::endl;
        for (const auto& scheme : dev.schemes) {
            for (const auto& line : scheme) {
                out << line << std::endl;
            }
        }
    };

    // 1. Find common devices (present in ALL files and Identical)
    std::ofstream common_out(common_output_file);
    if (!common_out.is_open()) {
        std::cerr << "Error: Could not open output file " << common_output_file << std::endl;
        return 1;
    }

    // We start with devices from the first file and check if they exist and are identical in all other files
    if (!all_files_devices.empty()) {
        for (const auto& pair : all_files_devices[0]) {
            const std::string& alias = pair.first;
            const Device& dev = pair.second;
            bool is_common = true;

            for (size_t i = 1; i < all_files_devices.size(); ++i) {
                if (all_files_devices[i].find(alias) == all_files_devices[i].end() ||
                    !(all_files_devices[i].at(alias) == dev)) {
                    is_common = false;
                    break;
                }
            }

            if (is_common) {
                write_device(common_out, dev);
            }
        }
    }
    common_out.close();

    // 2. Find unique devices for each file
    for (int i = 0; i < n; ++i) {
        std::ofstream unique_out(unique_output_files[i]);
        if (!unique_out.is_open()) {
            std::cerr << "Error: Could not open output file " << unique_output_files[i] << std::endl;
            return 1;
        }

        for (const auto& pair : all_files_devices[i]) {
            const std::string& alias = pair.first;
            const Device& dev = pair.second;
            
            bool unique_to_this_file = true;
            for (size_t k = 0; k < all_files_devices.size(); ++k) {
                if (static_cast<size_t>(i) == k) continue;
                if (all_files_devices[k].find(alias) != all_files_devices[k].end() &&
                    all_files_devices[k].at(alias) == dev) {
                    unique_to_this_file = false;
                    break;
                }
            }

            if (unique_to_this_file) {
                 write_device(unique_out, dev);
            }
        }
        unique_out.close();
    }

    return 0;
}
