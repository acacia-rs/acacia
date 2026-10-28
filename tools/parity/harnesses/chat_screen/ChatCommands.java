package acacia.parity.gui;

import com.google.gson.JsonObject;
import com.mojang.brigadier.CommandDispatcher;
import com.mojang.brigadier.StringReader;
import com.mojang.brigadier.arguments.ArgumentType;
import com.mojang.brigadier.arguments.BoolArgumentType;
import com.mojang.brigadier.arguments.FloatArgumentType;
import com.mojang.brigadier.arguments.IntegerArgumentType;
import com.mojang.brigadier.arguments.StringArgumentType;
import com.mojang.brigadier.builder.ArgumentBuilder;
import com.mojang.brigadier.builder.LiteralArgumentBuilder;
import com.mojang.brigadier.builder.RequiredArgumentBuilder;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.multiplayer.ClientSuggestionProvider;
import net.minecraft.commands.SharedSuggestionProvider;

/** The case's "commands" (acacia_ui::java::command::Tree::from_json's schema) as a CommandDispatcher. */
final class ChatCommands {
    private ChatCommands() {}

    /** Everything up to the next space: the Rust tree's type for Bedrock's untyped parameters. */
    static final class Token implements ArgumentType<String> {
        @Override public String parse(StringReader r) {
            int start = r.getCursor();
            while (r.canRead() && r.peek() != ' ') r.skip();
            return r.getString().substring(start, r.getCursor());
        }
    }

    @SuppressWarnings({"unchecked", "rawtypes"})
    static ArgumentBuilder<ClientSuggestionProvider, ?> node(JsonObject o) {
        ArgumentBuilder b;
        if (o.has("literal")) {
            b = LiteralArgumentBuilder.<ClientSuggestionProvider>literal(o.get("literal").getAsString());
        } else {
            ArgumentType<?> type = switch (o.has("type") ? o.get("type").getAsString() : "token") {
                case "integer" -> IntegerArgumentType.integer(o.has("min") ? o.get("min").getAsInt() : Integer.MIN_VALUE,
                                                              o.has("max") ? o.get("max").getAsInt() : Integer.MAX_VALUE);
                case "float" -> FloatArgumentType.floatArg(o.has("min") ? o.get("min").getAsFloat() : -Float.MAX_VALUE,
                                                           o.has("max") ? o.get("max").getAsFloat() : Float.MAX_VALUE);
                case "bool" -> BoolArgumentType.bool();
                case "word" -> StringArgumentType.word();
                case "string" -> StringArgumentType.string();
                case "greedy" -> StringArgumentType.greedyString();
                default -> new Token();
            };
            var a = RequiredArgumentBuilder.argument(o.get("argument").getAsString(), (ArgumentType) type);
            if (o.has("suggest")) {
                List<String> values = new ArrayList<>();
                for (var s : o.getAsJsonArray("suggest")) values.add(s.getAsString());
                a.suggests((ctx, builder) -> SharedSuggestionProvider.suggest(values, builder));
            }
            b = a;
        }
        if (o.has("exec") && o.get("exec").getAsBoolean()) b.executes(ctx -> 1);
        if (o.has("children")) for (var c : o.getAsJsonArray("children")) b.then(node(c.getAsJsonObject()));
        return b;
    }

    static CommandDispatcher<ClientSuggestionProvider> dispatcher(JsonObject c) {
        var d = new CommandDispatcher<ClientSuggestionProvider>();
        if (c.has("commands")) for (var n : c.getAsJsonArray("commands")) d.getRoot().addChild(node(n.getAsJsonObject()).build());
        return d;
    }
}
