package dev.horizonlab.parity;

import com.mojang.brigadier.CommandDispatcher;
import com.mojang.brigadier.tree.ArgumentCommandNode;
import com.mojang.brigadier.tree.CommandNode;
import java.io.PrintWriter;
import java.lang.reflect.Field;
import java.nio.file.Files;
import java.nio.file.Path;
import net.minecraft.commands.Commands;
import net.minecraft.commands.suggestions.CustomSuggestion;
import net.minecraft.commands.suggestions.SuggestionContext;
import net.minecraft.core.registries.BuiltInRegistries;

/**
 * Lists the argument nodes of vanilla's full command tree (Commands with
 * CommandSelection.ALL, the tree the commands report shows) that carry a
 * custom suggestion provider, with the provider's network name (the id of its
 * SerializableCustomSuggestion type in COMMAND_SUGGESTION_TYPE; ask_server for
 * providers with no registered type). One line per node: the path of node names
 * from the root, space separated, a tab, the provider id.
 */
public final class VanillaCommandSuggestionsExporter {
    public static void main(String[] args) throws Exception {
        ParityData.init();
        Commands commands = Commands.createVanilla(Commands.CommandSelection.ALL, Commands.createValidationContext(ParityData.lookup));
        Field dispatcherField = net.minecraft.commands.CommandBox.class.getDeclaredField("dispatcher");
        dispatcherField.setAccessible(true);
        @SuppressWarnings("unchecked")
        CommandDispatcher<SuggestionContext> dispatcher =
            (CommandDispatcher<SuggestionContext>) dispatcherField.get(commands.suggestionBox());
        try (PrintWriter out = new PrintWriter(Files.newBufferedWriter(Path.of(args[0])))) {
            walk(dispatcher.getRoot(), "", out);
        }
    }

    static void walk(CommandNode<?> node, String path, PrintWriter out) {
        for (CommandNode<?> child : node.getChildren()) {
            String childPath = path.isEmpty() ? child.getName() : path + " " + child.getName();
            if (child instanceof ArgumentCommandNode<?, ?> arg && arg.getCustomSuggestions() != null) {
                out.println(childPath + "\t" + providerName(arg.getCustomSuggestions()));
            }
            walk(child, childPath, out);
        }
    }

    static String providerName(com.mojang.brigadier.suggestion.SuggestionProvider<?> provider) {
        try {
            CustomSuggestion custom = CustomSuggestion.unwrapOrThrow(provider);
            var id = BuiltInRegistries.COMMAND_SUGGESTION_TYPE.getKey(custom.serializable().type());
            return id == null ? "minecraft:ask_server" : id.toString();
        } catch (RuntimeException e) {
            return "minecraft:ask_server";
        }
    }
}
