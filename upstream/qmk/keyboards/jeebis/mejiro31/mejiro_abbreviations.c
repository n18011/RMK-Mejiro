#include "mejiro_abbreviations.h"
#include <string.h>
#include <stdio.h>

// ユーザー略語のマッピング（完全なストローク形式）
typedef struct {
    const char *stroke;
    const char *output;
} user_abbreviation_t;

static const user_abbreviation_t user_abbreviations[] = {
  {"A-SKNIA*", "あめりか"},
  {"KNUntk-KNU*", "ぐーぐる"},
  {"KAUn-TAntk*", "こんぴゅーたー"},
  {"SKNIA-SAUtk*", "めそっど"},
  {"TNIA-SNI*", "でじたる"},
  {"SU-STKNAU*", "すまーとふぉん"},
  {"SU-TKAU*", "すまほ"},
  {"STKU-STA*", "ぷらすちっく"},
  {"KI-TKNAU*", "きーぼーど"},
  {"In-STA*", "いんふら"},
  {"KAUn-TKNIn*", "こんびに"},
  {"SI-KNAUt*", "しごと"},
  {"SI-SU*", "しすてむ"},
  {"SAU-TKUt*", "そふと"},
  {"SAU-SKIA*", "そふとうぇあ"},
  {"TKA-SKIA*", "はーどうぇあ"},
  {"In-NIAtk*", "いんたーねっと"},
  {"In-STKNAU*", "いんふぉめーしょん"},
  {"KAU-SKNYU*", "こみゅにけーしょん"},
  {"SI-SKNYU*", "しみゅれーしょん"},
  {"IU-STU*", "ういるす"},
  {"KAU-IU*", "ころなういるす"},
  {"SIn-KNAt*", "しんがた"},
  {"A-STI*", "ありがとう"},
  {"AU-NIA*", "おねがい"},
  {"YAU-STAU*", "よろしく"},
  {"TA-TAU*", "たとえば"},
  {"KNU-TY*", "ぐたいてき"},
  {"NIAntk-SAn*", "ねえさん"},
  {"#NIAntk-SAn*", "おねえさん"},
  {"NIntk-SAn*", "にいさん"},
  {"#NIntk-SAn*", "おにいさん"},
  {"KAntk-SAn*", "かあさん"},
  {"#KAntk-SAn*", "おかあさん"},
  {"TAUntk-SAn*", "とうさん"},
  {"#TAUntk-SAn*", "おとうさん"},
  {"TKNAntk-SAn*", "ばあさん"},
  {"#TKNAntk-SAn*", "おばあさん"},
  {"#TKNA-SAn*", "おばさん"},
  {"SNIntk-SAn*", "じいさん"},
  {"#SNIntk-SAn*", "おじいさん"},
  {"#SNI-SAn*", "おじさん"},
    {NULL, NULL}
};

// 一般略語の完全一致マッピング
typedef struct {
    const char *stroke;
    const char *output;
} abstract_abbreviation_t;

static const abstract_abbreviation_t abstract_abbreviations[] = {
  {"-STIA", "あれ"},
  {"-KAU", "あそこ"},
  {"-TI", "あっち"},
  {"-STA", "あちら"},
  {"ST-", "から"},
  {"K-N", "かん"},
  {"K-SN", "かんじ"},
  {"K-T", "こと"},
  {"KN-T", "ごと"},
  {"K-TKN", "ことば"},
  {"K-ST", "ころ"},
  {"K-STIA", "これ"},
  {"K-KAU", "ここ"},
  {"K-TI", "こっち"},
  {"K-STA", "こちら"},
  {"S-STIA", "それ"},
  {"S-KAU", "そこ"},
  {"S-TI", "そっち"},
  {"S-STA", "そちら"},
  {"T-TN", "ただ"},
  {"TN-K", "だけ"},
  {"TN-ST", "だから"},
  {"T-KI", "てき"},
  {"T-K", "とき"},
  {"T-ST", "ところ"},
  {"T-KAU", "とこ"},
  {"TN-STIA", "どれ"},
  {"TN-KAU", "どこ"},
  {"TN-TI", "どっち"},
  {"TN-STA", "どちら"},
  {"N-N", "なに"},
  {"NA-N", "なん"},
  {"TK-T", "ひと"},
  {"SKN-N", "もの"},
  {"SKNAU-N", "もん"},
  {"SK-K", "わけ"},
    {NULL, NULL}
};

// 一般略語の左側マッピング
typedef struct {
    const char *stroke;
    const char *output;
} abstract_left_t;

static const abstract_left_t abstract_left[] = {
    {"STN", ""},
    {"", "あの"},
    {"K", "この"},
    {"S", "その"},
    {"TN", "どの"},
    {"T", "との"},
    {"N", "なんの"},
    {"SKYU", "いう"},
    {"IU", "ああいう"},
    {"KIU", "こういう"},
    {"SIU", "そういう"},
    {"TNIU", "どういう"},
    {"TIU", "という"},
    {"NIU", "なんていう"},
    {NULL, NULL}
};

// 一般略語の右側マッピング
typedef struct {
    const char *stroke;
    const char *output;
} abstract_right_t;

static const abstract_right_t abstract_right[] = {
  {"STN", ""},
  {"AU", "おもい"},
  {"KA", "かんじ"},
  {"KNA", "かんがえ"},
  {"TI", "きもち"},
  {"KAU", "こと"},
  {"TKNA", "ことば"},
  {"STAU", "ころ"},
  {"SY", "さい"},
  {"SA", "さま"},
  {"SI", "しごと"},
  {"SYIA", "せい"},
  {"TA", "ため"},
  {"KNY", "ちがい"},
  {"TU", "つもり"},
  {"TYIA", "てい"},
  {"KI", "とき"},
  {"TAU", "ところ"},
  {"TKA", "はなし"},
  {"TKI", "ひと"},
  {"TKYIAU", "ふう"},
  {"TKIAU", "ほう"},
  {"SKNAU", "もの"},
  {"KNAU", "ものごと"},
  {"YI", "よう"},
    {NULL, NULL}
};

static void replace_nofuu_with_nnafuu(char *text, size_t text_size) {
    char *p = text;
    const size_t from_len = strlen("のふう");
    const size_t to_len = strlen("んなふう");
    while ((p = strstr(p, "のふう")) != NULL) {
        size_t tail_len = strlen(p + from_len);
        size_t used_len = strlen(text);
        if (used_len + (to_len - from_len) >= text_size) {
            break;
        }
        memmove(p + to_len, p + from_len, tail_len + 1);
        memcpy(p, "んなふう", to_len);
        p += to_len;
    }
}

// ユーザー略語を検索（完全なストローク）
abbreviation_result_t mejiro_user_abbreviation(const char *stroke, bool has_asterisk) {
    abbreviation_result_t result = {{0}, false};
    if (!has_asterisk) {
        return result;
    }
    char stroke_with_asterisk[64] = {0};
    snprintf(stroke_with_asterisk, sizeof(stroke_with_asterisk), "%s*", stroke);

    for (size_t i = 0; user_abbreviations[i].stroke != NULL; i++) {
        if (strcmp(stroke_with_asterisk, user_abbreviations[i].stroke) == 0) {
            strcpy(result.output, user_abbreviations[i].output);
            result.success = true;
            return result;
        }
    }

    return result;
}

// 一般略語を検索（完全なストローク）
abbreviation_result_t mejiro_abstract_abbreviation(const char *stroke, bool has_asterisk) {
    abbreviation_result_t result = {{0}, false};

    // asteriskなしは完全一致のみ（Python版 abstract_abbreviation_lookup と同等）
    if (!has_asterisk) {
        for (size_t i = 0; abstract_abbreviations[i].stroke != NULL; i++) {
            if (strcmp(stroke, abstract_abbreviations[i].stroke) == 0) {
                strcpy(result.output, abstract_abbreviations[i].output);
                result.success = true;
                return result;
            }
        }
        return result;
    }

    // asteriskありは左右の組み合わせをチェック
    // strokeを'-'で分割
    char left_part[32] = {0};
    char right_part[32] = {0};
    const char *hyphen = strchr(stroke, '-');
    if (hyphen != NULL) {
        size_t left_len = hyphen - stroke;
        strncpy(left_part, stroke, left_len);
        left_part[left_len] = '\0';
        strcpy(right_part, hyphen + 1);

        const char *left_output = NULL;
        const char *right_output = NULL;

        for (size_t i = 0; abstract_left[i].stroke != NULL; i++) {
            if (strcmp(left_part, abstract_left[i].stroke) == 0) {
                left_output = abstract_left[i].output;
                break;
            }
        }

        for (size_t i = 0; abstract_right[i].stroke != NULL; i++) {
            if (strcmp(right_part, abstract_right[i].stroke) == 0) {
                right_output = abstract_right[i].output;
                break;
            }
        }

        if (left_output != NULL && right_output != NULL) {
            strcpy(result.output, left_output);
            strcat(result.output, right_output);
            replace_nofuu_with_nnafuu(result.output, sizeof(result.output));
            result.success = true;
        }
    }

    return result;
}
