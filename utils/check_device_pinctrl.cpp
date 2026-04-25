#include <iostream>
#include <fstream>
#include <sstream>
#include <string>
#include <vector>
#include <map>
#include <set>
#include <algorithm>

struct PinKey {
    int bank;
    int pin;

    bool operator<(const PinKey& other) const {
        if (bank != other.bank) return bank < other.bank;
        return pin < other.pin;
    }
};

struct DeviceEntry {
    std::string device_name;
    std::string source_file;

    bool operator<(const DeviceEntry& other) const {
        if (device_name != other.device_name) return device_name < other.device_name;
        return source_file < other.source_file;
    }
};

struct PinInfo {
    PinKey key;
    std::set<DeviceEntry> devices;
};

int main(int argc, char* argv[]) {
    if (argc < 2) {
        std::cerr << "Usage: " << argv[0] << " <input_file> [input_file2 ...]" << std::endl;
        return 1;
    }

    std::map<PinKey, std::set<DeviceEntry>> pin_map;

    for (int i = 1; i < argc; ++i) {
        std::string filename = argv[i];
        // Only keep basename
        size_t last_slash = filename.find_last_of("/\\");
        std::string basename = (last_slash == std::string::npos) ? filename : filename.substr(last_slash + 1);

        std::ifstream infile(filename);
        if (!infile.is_open()) {
            std::cerr << "Error opening file: " << filename << std::endl;
            continue;
        }

        std::string line;
        while (std::getline(infile, line)) {
            if (line.empty()) continue;
            std::stringstream ss(line);
            std::string segment;
            std::vector<std::string> parts;

            while (std::getline(ss, segment, ',')) {
                parts.push_back(segment);
            }

            if (parts.size() < 4) continue;

            // DeviceName,PinctrlName,Bank,Pin,Mux,Config
            std::string device_name = parts[0];
            try {
                int bank = std::stoi(parts[2]);
                int pin = std::stoi(parts[3]);
                pin_map[{bank, pin}].insert({device_name, basename});
            } catch (...) {
                continue;
            }
        }
    }

    std::vector<PinInfo> sorted_pins;
    for (const auto& pair : pin_map) {
        sorted_pins.push_back({pair.first, pair.second});
    }

    // Sort by device count (small to large)
    std::sort(sorted_pins.begin(), sorted_pins.end(), [](const PinInfo& a, const PinInfo& b) {
        if (a.devices.size() != b.devices.size()) {
            return a.devices.size() < b.devices.size();
        }
        // Secondary sort by bank/pin for stability
        return a.key < b.key;
    });

    for (const auto& info : sorted_pins) {
        std::cout << "gpio" << info.key.bank << "-" << info.key.pin << " (" << info.devices.size() << " devices): ";
        bool first = true;
        for (const auto& dev : info.devices) {
            if (!first) std::cout << ", ";
            std::cout << dev.device_name << "(" << dev.source_file << ")";
            first = false;
        }
        std::cout << std::endl;
    }

    return 0;
}
