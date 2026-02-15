// Ghidra headless script to decompile key Scribius functions
// Run with analyzeHeadless -postScript ScribiusDecompile.java
//@category Analysis

import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionIterator;
import ghidra.program.model.data.StringDataInstance;
import ghidra.program.model.listing.Data;
import ghidra.program.model.listing.DataIterator;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.ReferenceIterator;
import ghidra.program.model.address.Address;

import java.util.ArrayList;
import java.util.List;

public class ScribiusDecompile extends GhidraScript {

    // Objective-C selectors we want to decompile
    private static final String[] TARGET_SELECTORS = {
        "quickProfessionForTrainer:",
        "fastCacheProfessionDictionary",
        "determineProfessionForCharacter:",
        "detectPetInfo",
        "inputLastyForChar",
        "updateLastyForCharacter",
        "prepareLastysForCharacter",
        "coinLevel",
        "parsePetInfo",
        "petInfo",
        "scanForLasty",
        "scanForPet",
        "processPet",
        "parseLine",
        "scanLine",
        "processLine",
        "profession",
        "setProfession",
        "setCoinLevel",
        "computeCoinLevel",
        "calculateCoinLevel",
    };

    // String patterns to search for cross-references
    private static final String[] TARGET_STRINGS = {
        "You learn to befriend the",
        "You learn to",
        "You have completed your training with",
        "coinLevel",
        "Coin Level",
        "coin_level",
        "profession",
        "Profession",
        "Fighter",
        "Healer",
        "Mystic",
        "Ranger",
        "befriend",
        "morph",
        "Morph",
        "movements",
        "Movements",
        "Befriend",
        "You sense a",
        "You feel a",
        "pet",
        "Pet",
        "lasty",
        "Lasty",
    };

    @Override
    public void run() throws Exception {
        println("=== SCRIBIUS DECOMPILATION SCRIPT ===");
        println("Program: " + currentProgram.getName());
        println("");

        DecompInterface decomp = new DecompInterface();
        decomp.openProgram(currentProgram);

        // Phase 1: Find and decompile functions matching target selectors
        println("=== PHASE 1: DECOMPILING TARGET FUNCTIONS ===");
        println("");

        FunctionIterator funcIter = currentProgram.getFunctionManager().getFunctions(true);
        List<Function> matchedFunctions = new ArrayList<>();

        while (funcIter.hasNext()) {
            Function func = funcIter.next();
            String name = func.getName();

            for (String selector : TARGET_SELECTORS) {
                if (name.contains(selector) || name.toLowerCase().contains(selector.toLowerCase())) {
                    matchedFunctions.add(func);
                    break;
                }
            }
        }

        println("Found " + matchedFunctions.size() + " matching functions:");
        for (Function func : matchedFunctions) {
            println("  - " + func.getName() + " @ " + func.getEntryPoint());
        }
        println("");

        for (Function func : matchedFunctions) {
            println("--- DECOMPILING: " + func.getName() + " @ " + func.getEntryPoint() + " ---");
            DecompileResults results = decomp.decompileFunction(func, 60, monitor);
            if (results.decompileCompleted()) {
                println(results.getDecompiledFunction().getC());
            } else {
                println("FAILED to decompile: " + results.getErrorMessage());
            }
            println("--- END: " + func.getName() + " ---");
            println("");
        }

        // Phase 2: Also search for any function with "profession", "pet", "lasty", "coin" in name
        println("=== PHASE 2: BROADER FUNCTION NAME SEARCH ===");
        println("");

        String[] broadTerms = {"profession", "pet", "lasty", "coin", "befriend", "morph", "movement"};
        funcIter = currentProgram.getFunctionManager().getFunctions(true);
        List<Function> broadMatches = new ArrayList<>();

        while (funcIter.hasNext()) {
            Function func = funcIter.next();
            String nameLower = func.getName().toLowerCase();

            for (String term : broadTerms) {
                if (nameLower.contains(term)) {
                    // Avoid duplicates with Phase 1
                    boolean alreadyFound = false;
                    for (Function mf : matchedFunctions) {
                        if (mf.getEntryPoint().equals(func.getEntryPoint())) {
                            alreadyFound = true;
                            break;
                        }
                    }
                    if (!alreadyFound) {
                        broadMatches.add(func);
                    }
                    break;
                }
            }
        }

        println("Found " + broadMatches.size() + " additional broad matches:");
        for (Function func : broadMatches) {
            println("  - " + func.getName() + " @ " + func.getEntryPoint());
        }
        println("");

        for (Function func : broadMatches) {
            println("--- DECOMPILING: " + func.getName() + " @ " + func.getEntryPoint() + " ---");
            DecompileResults results = decomp.decompileFunction(func, 60, monitor);
            if (results.decompileCompleted()) {
                println(results.getDecompiledFunction().getC());
            } else {
                println("FAILED to decompile: " + results.getErrorMessage());
            }
            println("--- END: " + func.getName() + " ---");
            println("");
        }

        // Phase 3: Find strings and their cross-references
        println("=== PHASE 3: STRING CROSS-REFERENCES ===");
        println("");

        DataIterator dataIter = currentProgram.getListing().getDefinedData(true);
        while (dataIter.hasNext()) {
            Data data = dataIter.next();
            if (data.hasStringValue()) {
                String value = data.getDefaultValueRepresentation();
                // Strip quotes for comparison
                String cleanValue = value;
                if (cleanValue.startsWith("\"") && cleanValue.endsWith("\"")) {
                    cleanValue = cleanValue.substring(1, cleanValue.length() - 1);
                }

                for (String target : TARGET_STRINGS) {
                    if (cleanValue.contains(target)) {
                        println("STRING FOUND: " + value + " @ " + data.getAddress());

                        // Find cross-references to this string
                        Reference[] refs = getReferencesTo(data.getAddress());
                        if (refs.length > 0) {
                            for (Reference ref : refs) {
                                Address fromAddr = ref.getFromAddress();
                                Function refFunc = getFunctionContaining(fromAddr);
                                if (refFunc != null) {
                                    println("  Referenced from: " + refFunc.getName() + " @ " + fromAddr);

                                    // Decompile the referencing function
                                    println("  --- DECOMPILING REFERENCING FUNCTION: " + refFunc.getName() + " ---");
                                    DecompileResults results = decomp.decompileFunction(refFunc, 60, monitor);
                                    if (results.decompileCompleted()) {
                                        println(results.getDecompiledFunction().getC());
                                    } else {
                                        println("  FAILED to decompile: " + results.getErrorMessage());
                                    }
                                    println("  --- END REFERENCING FUNCTION ---");
                                } else {
                                    println("  Referenced from non-function address: " + fromAddr);
                                }
                            }
                        } else {
                            // Try references to the pointer to this data
                            println("  No direct references found, checking pointer references...");
                        }
                        println("");
                        break;
                    }
                }
            }
        }

        // Phase 4: List ALL Objective-C methods (look for bracket naming convention)
        println("=== PHASE 4: ALL OBJC METHODS WITH KEY TERMS ===");
        println("");
        funcIter = currentProgram.getFunctionManager().getFunctions(true);
        while (funcIter.hasNext()) {
            Function func = funcIter.next();
            String name = func.getName();
            // Objective-C methods often contain brackets or underscores with class names
            String nameLower = name.toLowerCase();
            if ((nameLower.contains("profession") || nameLower.contains("pet") ||
                 nameLower.contains("lasty") || nameLower.contains("coin") ||
                 nameLower.contains("befriend") || nameLower.contains("morph") ||
                 nameLower.contains("movement") || nameLower.contains("trainer") ||
                 nameLower.contains("scan") || nameLower.contains("parse"))) {
                println("OBJC METHOD: " + name + " @ " + func.getEntryPoint());
            }
        }

        println("");
        println("=== DECOMPILATION COMPLETE ===");

        decomp.dispose();
    }
}
