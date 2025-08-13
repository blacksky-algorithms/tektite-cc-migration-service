Manual Account Migration

In some situations the automatic process might not be possible or desirable. These more manual commands give more control over the process, work with different DID methods, give more direct control over the final DID document, result in local backups of all public data (and private preferences), and may allow recovery if the original PDS is down or uncooperative (assuming self-control of the DID).

These steps require flipping back and forth between goat logged in to the old PDS and the new PDS. Instead of showing those goat account login / logout commands, these directions will indicate which PDS should be authenticated at each step.

You can fetch metadata about a PDS, including the service DID and supported handle suffix:

```
# no auth required
goat pds describe $NEWPDSHOST

# for example
goat pds describe https://bsky.social
{
  "availableUserDomains": [
    ".bsky.social"
  ],
  "did": "did:web:bsky.social",
  "inviteCodeRequired": false,
  "links": {
    "privacyPolicy": "https://blueskyweb.xyz/support/privacy-policy",
    "termsOfService": "https://blueskyweb.xyz/support/tos"
  },
  "phoneVerificationRequired": true
}
```

To create an account with an existing DID on the new PDS, we first need to generate a service auth token:

```
# old PDS
goat account service-auth --lxm com.atproto.server.createAccount --aud $NEWPDSSERVICEDID --duration-sec 3600

This returns a large base64-encoded token ($SERVICEAUTH).

Now an account can be created on the new PDS, using the existing DID:

# no auth
goat account create \
    --pds-host $NEWPDSHOST \
    --existing-did $ACCOUNTDID \
    --handle $NEWHANDLE \
    --password $NEWPASSWORD \
    --email $NEWEMAIL \
    --invite-code $INVITECODE \
    --service-auth $SERVICEAUTH
```

The new account will be "deactivated", because the identity (DID) does not point to this PDS host yet. To log in to an account when the DID doesn't resolve yet, goat requires specifying the PDS host:

goat account login --pds-host $NEWPDSHOST -u $ACCOUNTDID -p $NEWPASSWORD

You can check the current account status like:

```
# new PDS
goat account status
{
    "activated": false,
    "expectedBlobs": 0,
    "importedBlobs": 0,
    "indexedRecords": 0,
    "privateStateValues": 0,
    "repoBlocks": 2,
    "repoCommit": "bafyreie2o6idkbnpkhkwp6ocf7p5k7np2t7xnx3346zqc456f3avhsnhue",
    "repoRev": "3l5ddasaitk23",
    "validDid": false
}
```
Next to migrate content, starting with repo:


```
# old PDS
goat repo export $ACCOUNTDID

# will write a CAR file like ./did:plc:do2ar6uqzrvyzq3wevji6fbe.20250625142552.car

# new PDS
goat repo import ./did:plc:do2ar6uqzrvyzq3wevji6fbe.20250625142552.car

Once all the old records are indexed, the new PDS will know how many blobs are expected (expectedBlobs in account status), and how many have been imported (importedBlobs). You can also check the specific "missing" blobs:

# new PDS
goat account missing-blobs

# example output:
# bafkreibyu5mlurlwyjj2ewfjddmm7euiq47xisdyf4sil46s2zu4bultiu	at://did:plc:c7ilkj3gs7mdo3d6vdbebgk2/app.bsky.actor.profile/self
# bafkreieymnbzgpcjdebyjewy3z7jmpqg6h3uf5fl4khuywz65tgmknvlgu	at://did:plc:c7ilkj3gs7mdo3d6vdbebgk2/app.bsky.feed.post/3l5cs7sszcx2s
# [...]
```

To export and import all blobs:

```
# old PDS
goat blob export $ACCOUNTDID

# will create a directory like ./account_blobs/

# new PDS
# this requires the 'fd' (fd-find) and 'parallel' commands
fd . ./account_blobs/ | parallel -j1 goat blob upload {}
```

You can confirm that there are no missing blobs, and that the blob and record counts match the old PDS.

Next, private Bluesky app preferences.

As a warning, the current Go code for serializing/deserializing preferences may be "lossy" if the preference schemas are out of sync or for non-Bluesky Lexicons, and it is possible this step will lose some preference metadata. This will hopefully be improved in a future version of goat, or when the preferences API is updated to be app-agnostic ("personal data" protocol support).

```
# old PDS
goat bsky prefs export > prefs.json

# new PDS
goat bsky prefs import prefs.json
```

With all the content migrated to the new account, we can update the identity (DID) to point at the new PDS instance.

Fetch the "recommended" DID parameters from the new PDS:

```
# new PDS
goat account plc recommended > plc_recommended.json
```
If you are self-managing your identity (eg, did:web or self-controlled did:plc), you can merge these parameters in to your DID document.

If using a PDS-managed did:plc, you can edit the parameters to match any additional services or recovery keys. Save the results as ./plc_unsigned.json. You will need to request a PLC signing token from the PDS:
```
# old PDS
goat account plc request-token
```

Retrieve the token ($PLCTOKEN) from email, then request a signed version of the PLC params:

```
# old PDS
goat account plc sign --token $PLCTOKEN  ./plc_unsigned.json > plc_signed.json
```

If that looks good, the PLC Op can be submitted from the new PDS:

```
# new PDS
goat account plc submit ./plc_signed.json
```

Check the account status on the new PDS, and validDid should now be true.

As the final steps, the new PDS account can be activated:

```
# new PDS
goat account activate
```

and the old PDS account deactivated:

```
# old PDS
goat account deactivate
```
You may chose to delete the old account once you are confident the new account is configured and running as expected.